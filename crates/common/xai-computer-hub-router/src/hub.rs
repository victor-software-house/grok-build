//! Hub state and frame routing.
//!
//! All routing follows the SDK demux contract
//! (`xai-computer-hub-sdk/src/demux.rs`):
//!
//! 1. responses correlate by envelope `id`;
//! 2. `tool_call_progress` correlates by `params.tool_call_id`;
//! 3. requests/notifications WITH an envelope `session_id` land in the
//!    receiver's per-session inbox (exists only after bind);
//! 4. frames WITHOUT a `session_id` land on the connection-level
//!    broadcast (`ToolServer::run` handles `session.bind` there and
//!    reads `params.session_id`).
//!
//! Consequently `session.bind`/`session.unbind` toward a server carry
//! the session id in PARAMS and no envelope `session_id`, while
//! `tool_call_request` carries the envelope `session_id` verbatim.
//! (NOTE: `frames.rs` documents an empty `SessionBindParams` with the
//! session id on the envelope, but the deployed `ToolServer` run loop
//! consumes `session.bind` exclusively from the connection broadcast
//! and reads `/params/session_id` — sdk/server.rs:1372-1389. The
//! router speaks the shape the shipped binaries implement.)
//!
//! JSON-RPC ids are per-connection and sender-allocated, so every
//! forwarded request gets a router-minted id; the response is matched
//! by that minted id and re-enveloped with the origin's id.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;
use xai_tool_protocol::{
    ConnectionKind, HelloAckMsg, HelloMsg, JsonRpcError, PROTOCOL_VERSION, ServerId,
    ServerInfo, ServersListResult, SessionId, ToolServerLifecycleStatus, UserId,
    frames::PongFrame,
};

/// Wire methods the router answers or translates itself; everything else
/// on a harness session is forwarded to the bound server.
const HUB_LOCAL_METHODS: &[&str] = &[
    "session_open",
    "session_close",
    "session_bind_server",
    "session_unbind_server",
    "tools.list",
    "servers.list",
    "subscribe_notifications",
    "unsubscribe_notifications",
];

/// Numeric key for a live connection.
pub(crate) type ConnKey = u64;

pub(crate) struct ConnEntry {
    pub user: UserId,
    pub kind: ConnectionKind,
    /// Stable identity for tool-server connections.
    pub server_id: Option<ServerId>,
    pub description: String,
    pub metadata: Value,
    pub connected_since: String,
    pub tx: UnboundedSender<String>,
}

#[derive(Default)]
struct SessionEntry {
    /// Harness connection that opened the session.
    owner: Option<ConnKey>,
    /// Tool-server connection currently bound to the session.
    server: Option<ConnKey>,
    /// Tool snapshot from the last `session.bind` result / `serve`.
    tools: Value,
}

struct Pending {
    origin: ConnKey,
    origin_id: Value,
    /// `session_bind_server` replies need result-shape mapping and
    /// binding bookkeeping on success.
    bind: Option<(SessionId, ConnKey)>,
}

#[derive(Default)]
struct HubInner {
    conns: HashMap<ConnKey, ConnEntry>,
    servers: HashMap<(String, ServerId), ConnKey>,
    sessions: HashMap<SessionId, SessionEntry>,
    pending: HashMap<String, Pending>,
}

/// Shared router state. One instance per listener.
#[derive(Default)]
pub struct Hub {
    inner: Mutex<HubInner>,
    ids: AtomicU64,
}

/// What the connection actor should do with an inbound frame.
enum Outbound {
    /// Send `frame` to connection `to`.
    To(ConnKey, Value),
    /// Reply on the originating connection.
    Reply(Value),
    /// Silently drop.
    Drop,
}

impl Hub {
    /// Register a connection after a successful handshake.
    /// Returns the `hello_ack` to send, or an error string that closes
    /// the socket (duplicate server id / bad hello).
    pub(crate) fn register(
        &self,
        key: ConnKey,
        user: UserId,
        hello: &HelloMsg,
        tx: UnboundedSender<String>,
    ) -> Result<HelloAckMsg, String> {
        if hello.protocol_version != PROTOCOL_VERSION {
            return Err(format!(
                "unsupported protocol version {}",
                hello.protocol_version
            ));
        }
        let server_id = match hello.kind {
            ConnectionKind::ToolServer => match &hello.server_id {
                Some(id) => Some(id.clone()),
                None => return Err("tool_server hello missing server_id".into()),
            },
            ConnectionKind::Harness => None,
        };
        let mut inner = self.inner.lock();
        if let Some(id) = &server_id {
            let slot = (user.as_str().to_owned(), id.clone());
            if let Some(prev) = inner.servers.insert(slot, key) {
                // Last write wins (reconnect); drop the stale registration.
                tracing::info!(server = %id, prev_conn = prev, "server re-registered");
            }
        }
        inner.conns.insert(
            key,
            ConnEntry {
                user: user.clone(),
                kind: hello.kind,
                server_id,
                description: hello.description.clone().unwrap_or_default(),
                metadata: hello.metadata.clone().unwrap_or(Value::Null),
                connected_since: now_rfc3339(),
                tx,
            },
        );
        let connection_id = format!("conn-{key}");
        Ok(HelloAckMsg {
            connection_id: xai_tool_protocol::ConnectionId::new(&connection_id)
                .expect("generated connection id is valid"),
            user_id: user,
            computer_hub_version: env!("CARGO_PKG_VERSION").to_owned(),
            supported_protocol_versions: vec![PROTOCOL_VERSION.to_owned()],
            capabilities: Vec::new(),
        })
    }

    /// Tear down a connection: fail pendings targeting it, unbind its
    /// sessions, drop registrations.
    pub(crate) fn disconnect(&self, key: ConnKey) {
        let mut replies: Vec<(ConnKey, Value)> = Vec::new();
        let mut unbinds: Vec<(ConnKey, Value)> = Vec::new();
        {
            let mut inner = self.inner.lock();
            let Some(entry) = inner.conns.remove(&key) else {
                return;
            };
            if let Some(id) = &entry.server_id {
                let slot = (entry.user.as_str().to_owned(), id.clone());
                if inner.servers.get(&slot) == Some(&key) {
                    inner.servers.remove(&slot);
                }
            }
            // Requests initiated by this connection can never be answered.
            inner.pending.retain(|_, p| p.origin != key);
            // Sessions: harness owner gone → unbind server; server gone →
            // mark sessions unbound.
            let session_ids: Vec<SessionId> = inner.sessions.keys().cloned().collect();
            for sid in session_ids {
                let Some(sess) = inner.sessions.get_mut(&sid) else {
                    continue;
                };
                if sess.owner == Some(key) {
                    if let Some(server_key) = sess.server {
                        unbinds.push((server_key, session_unbind_frame(&sid)));
                    }
                    inner.sessions.remove(&sid);
                } else if sess.server == Some(key) {
                    sess.server = None;
                    sess.tools = Value::Null;
                }
            }
            // Requests still pending toward the dead connection get an
            // error response back to their origin.
            let dead: Vec<String> = inner
                .pending
                .iter()
                .filter(|(minted, _)| minted.starts_with(&format!("hub-{key}-")))
                .map(|(minted, _)| minted.clone())
                .collect();
            for minted in dead {
                if let Some(p) = inner.pending.remove(&minted) {
                    replies.push((
                        p.origin,
                        error_response(p.origin_id, -32603, "peer disconnected"),
                    ));
                }
            }
            for (to, frame) in replies.drain(..) {
                send_locked(&inner, to, &frame);
            }
            for (to, frame) in unbinds.drain(..) {
                send_locked(&inner, to, &frame);
            }
        }
        tracing::info!(conn = key, "connection closed");
    }

    /// Route one inbound text frame from connection `key`.
    pub(crate) fn route(&self, key: ConnKey, raw: &str) {
        let Ok(frame) = serde_json::from_str::<Value>(raw) else {
            tracing::warn!(conn = key, "dropping unparsable frame");
            return;
        };
        let outbound = self.classify(key, frame);
        let inner = self.inner.lock();
        match outbound {
            Outbound::To(to, frame) => send_locked(&inner, to, &frame),
            Outbound::Reply(frame) => send_locked(&inner, key, &frame),
            Outbound::Drop => {}
        }
    }

    fn classify(&self, key: ConnKey, frame: Value) -> Outbound {
        let method = frame.get("method").and_then(Value::as_str);
        match method {
            Some("ping") => {
                let ts = frame.get("ts_ms").and_then(Value::as_u64).unwrap_or(0);
                Outbound::Reply(
                    serde_json::to_value(PongFrame { ts_ms: ts }).expect("pong serializes"),
                )
            }
            Some("pong") => Outbound::Drop,
            Some(m) => self.classify_method(key, m.to_owned(), frame),
            // No method → a response; correlate by minted id.
            None => self.correlate_response(frame),
        }
    }

    fn classify_method(&self, key: ConnKey, method: String, frame: Value) -> Outbound {
        let kind = {
            let inner = self.inner.lock();
            match inner.conns.get(&key) {
                Some(c) => c.kind,
                None => return Outbound::Drop,
            }
        };
        match kind {
            ConnectionKind::Harness => self.harness_frame(key, &method, frame),
            ConnectionKind::ToolServer => self.server_frame(key, &method, frame),
        }
    }

    // ── Harness-originated frames ─────────────────────────────────────

    fn harness_frame(&self, key: ConnKey, method: &str, frame: Value) -> Outbound {
        let id = frame.get("id").cloned();
        let sid = envelope_session(&frame);
        if HUB_LOCAL_METHODS.contains(&method) {
            return self.hub_local(key, method, id, sid, frame);
        }
        // Everything else needs a bound server for the envelope session.
        let Some(sid) = sid else {
            return reply_error(id, -32600, "missing session_id");
        };
        let inner = self.inner.lock();
        let Some(server_key) = inner.sessions.get(&sid).and_then(|s| s.server) else {
            return reply_error(id, -32005, "no tool server bound for session");
        };
        let mut fwd = frame;
        if method == "tool.call" {
            fwd["method"] = Value::String("tool_call_request".to_owned());
        }
        match id {
            Some(origin_id) => {
                drop(inner);
                let minted = self.mint(server_key, key, origin_id, None);
                fwd["id"] = Value::String(minted);
                Outbound::To(server_key, fwd)
            }
            None => Outbound::To(server_key, fwd),
        }
    }

    fn hub_local(
        &self,
        key: ConnKey,
        method: &str,
        id: Option<Value>,
        sid: Option<SessionId>,
        frame: Value,
    ) -> Outbound {
        match method {
            "session_open" => {
                let Some(sid) = sid else {
                    return reply_error(id, -32600, "session_open missing session_id");
                };
                let mut inner = self.inner.lock();
                let sess = inner.sessions.entry(sid).or_default();
                sess.owner = Some(key);
                reply_ok(id, json!({}))
            }
            "session_close" => {
                let Some(sid) = sid else {
                    return reply_error(id, -32600, "session_close missing session_id");
                };
                let mut inner = self.inner.lock();
                if let Some(sess) = inner.sessions.remove(&sid)
                    && let Some(server_key) = sess.server
                {
                    send_locked(&inner, server_key, &session_unbind_frame(&sid));
                }
                reply_ok(id, json!({}))
            }
            "session_bind_server" => self.bind_server(key, id, sid, &frame),
            "session_unbind_server" => {
                let Some(sid) = sid else {
                    return reply_error(id, -32600, "session_unbind_server missing session_id");
                };
                let mut inner = self.inner.lock();
                if let Some(sess) = inner.sessions.get_mut(&sid)
                    && let Some(server_key) = sess.server.take()
                {
                    sess.tools = Value::Null;
                    send_locked(&inner, server_key, &session_unbind_frame(&sid));
                }
                reply_ok(id, json!({}))
            }
            "tools.list" => {
                let session = frame
                    .pointer("/params/session_id")
                    .and_then(Value::as_str)
                    .and_then(|s| SessionId::new(s).ok())
                    .or(sid);
                let inner = self.inner.lock();
                let tools = session
                    .and_then(|s| inner.sessions.get(&s))
                    .map(|s| s.tools.clone())
                    .filter(|t| !t.is_null())
                    .unwrap_or_else(|| json!([]));
                reply_ok(id, json!({ "tools": tools }))
            }
            "servers.list" => {
                let inner = self.inner.lock();
                let user = match inner.conns.get(&key) {
                    Some(c) => c.user.clone(),
                    None => return Outbound::Drop,
                };
                let servers: Vec<ServerInfo> = inner
                    .servers
                    .iter()
                    .filter(|((owner, _), _)| owner == user.as_str())
                    .filter_map(|((_, server_id), conn_key)| {
                        let conn = inner.conns.get(conn_key)?;
                        Some(ServerInfo {
                            server_id: server_id.clone(),
                            session_id: None,
                            description: conn.description.clone(),
                            metadata: conn.metadata.clone(),
                            connected_since: conn.connected_since.clone(),
                            status: ToolServerLifecycleStatus::Ready,
                        })
                    })
                    .collect();
                let result = serde_json::to_value(ServersListResult { servers })
                    .expect("servers list serializes");
                reply_ok(id, result)
            }
            "subscribe_notifications" => reply_ok(
                id,
                json!({ "outcome": "subscribed", "subscription_id": "default" }),
            ),
            "unsubscribe_notifications" => reply_ok(
                id,
                json!({ "outcome": "unsubscribed", "subscription_id": "default" }),
            ),
            _ => unreachable!("HUB_LOCAL_METHODS is exhaustive"),
        }
    }

    fn bind_server(
        &self,
        key: ConnKey,
        id: Option<Value>,
        sid: Option<SessionId>,
        frame: &Value,
    ) -> Outbound {
        let Some(sid) = sid else {
            return reply_error(id, -32600, "session_bind_server missing session_id");
        };
        let Some(id) = id else {
            return Outbound::Drop; // bind must be a request
        };
        let Some(server_id) = frame
            .pointer("/params/server_id")
            .and_then(Value::as_str)
            .and_then(|s| ServerId::new(s).ok())
        else {
            return reply_error(Some(id), -32600, "session_bind_server missing server_id");
        };
        let server_key = {
            let mut inner = self.inner.lock();
            let user = match inner.conns.get(&key) {
                Some(c) => c.user.as_str().to_owned(),
                None => return Outbound::Drop,
            };
            let Some(&server_key) = inner.servers.get(&(user, server_id.clone())) else {
                return reply_error(
                    Some(id),
                    -32602,
                    &format!("unknown server `{server_id}`"),
                );
            };
            let sess = inner.sessions.entry(sid.clone()).or_default();
            sess.owner = Some(key);
            server_key
        };
        // session.bind → server: session id in PARAMS, no envelope sid
        // (must land on the server's connection-level broadcast).
        let mut params = json!({ "session_id": sid.as_str() });
        if let Some(cwd) = frame.pointer("/params/cwd") {
            params["cwd"] = cwd.clone();
        }
        if let Some(metadata) = frame.pointer("/params/metadata") {
            params["metadata"] = metadata.clone();
        }
        let minted = self.mint(server_key, key, id, Some((sid, server_key)));
        Outbound::To(
            server_key,
            json!({
                "jsonrpc": "2.0",
                "id": minted,
                "method": "session.bind",
                "params": params,
            }),
        )
    }

    // ── Server-originated frames ──────────────────────────────────────

    fn server_frame(&self, key: ConnKey, method: &str, frame: Value) -> Outbound {
        let id = frame.get("id").cloned();
        match method {
            "serve" => {
                let Some(sid) = envelope_session(&frame) else {
                    return reply_error(id, -32600, "serve missing session_id");
                };
                let tools = frame
                    .pointer("/params/tools")
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                let accepted = tools.as_array().map(Vec::len).unwrap_or(0);
                let mut inner = self.inner.lock();
                let sess = inner.sessions.entry(sid).or_default();
                sess.server = Some(key);
                sess.tools = strip_schemas(tools);
                reply_ok(id, json!({ "accepted": accepted }))
            }
            "traces.donate" | "logs.donate" | "metrics.donate" | "tool_server.status" => {
                Outbound::Drop
            }
            // Progress, notifications, hooks: route to the session owner.
            _ => {
                let Some(sid) = envelope_session(&frame) else {
                    tracing::debug!(method, "dropping server frame without session");
                    return Outbound::Drop;
                };
                let inner = self.inner.lock();
                let Some(owner) = inner.sessions.get(&sid).and_then(|s| s.owner) else {
                    return Outbound::Drop;
                };
                match id {
                    Some(origin_id) => {
                        drop(inner);
                        let minted = self.mint(owner, key, origin_id, None);
                        let mut fwd = frame;
                        fwd["id"] = Value::String(minted);
                        Outbound::To(owner, fwd)
                    }
                    None => Outbound::To(owner, frame),
                }
            }
        }
    }

    // ── Response correlation ──────────────────────────────────────────

    fn correlate_response(&self, mut frame: Value) -> Outbound {
        let Some(minted) = frame.get("id").and_then(Value::as_str).map(str::to_owned) else {
            return Outbound::Drop;
        };
        let pending = {
            let mut inner = self.inner.lock();
            inner.pending.remove(&minted)
        };
        let Some(p) = pending else {
            tracing::debug!(id = %minted, "dropping unmatched response");
            return Outbound::Drop;
        };
        if let Some((sid, server_key)) = p.bind {
            // session.bind result → session_bind_server result: shapes are
            // field-compatible (tools/binary_version/unserved_tool_ids/
            // resolve_error), so relay the body and record the binding.
            if frame.get("result").is_some() {
                let mut inner = self.inner.lock();
                let sess = inner.sessions.entry(sid).or_default();
                sess.server = Some(server_key);
                sess.tools = strip_schemas(
                    frame
                        .pointer("/result/tools")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                );
            }
        }
        frame["id"] = p.origin_id;
        Outbound::To(p.origin, frame)
    }

    /// Mint a correlator toward `target` for a request from `origin`.
    fn mint(
        &self,
        target: ConnKey,
        origin: ConnKey,
        origin_id: Value,
        bind: Option<(SessionId, ConnKey)>,
    ) -> String {
        let n = self.ids.fetch_add(1, Ordering::Relaxed);
        let minted = format!("hub-{target}-{n}");
        self.inner.lock().pending.insert(
            minted.clone(),
            Pending {
                origin,
                origin_id,
                bind,
            },
        );
        minted
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn envelope_session(frame: &Value) -> Option<SessionId> {
    frame
        .get("session_id")
        .and_then(Value::as_str)
        .and_then(|s| SessionId::new(s).ok())
}

fn session_unbind_frame(sid: &SessionId) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "session.unbind",
        "params": { "session_id": sid.as_str() },
    })
}

fn error_response(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": JsonRpcError { code, message: message.to_owned(), data: None },
    })
}

fn reply_ok(id: Option<Value>, result: Value) -> Outbound {
    match id {
        Some(id) => Outbound::Reply(json!({ "jsonrpc": "2.0", "id": id, "result": result })),
        None => Outbound::Drop,
    }
}

fn reply_error(id: Option<Value>, code: i32, message: &str) -> Outbound {
    match id {
        Some(id) => Outbound::Reply(error_response(id, code, message)),
        None => Outbound::Drop,
    }
}

/// `serve`/`session.bind` snapshots carry `ToolDescriptionWithSchema`;
/// `tools.list` replies carry plain `ToolDescription`s. Dropping the
/// `schema` field is the projection between the two.
fn strip_schemas(tools: Value) -> Value {
    match tools {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|mut t| {
                    if let Some(obj) = t.as_object_mut() {
                        obj.remove("schema");
                    }
                    t
                })
                .collect(),
        ),
        other => other,
    }
}

fn send_locked(inner: &HubInner, to: ConnKey, frame: &Value) {
    let Some(conn) = inner.conns.get(&to) else {
        tracing::debug!(conn = to, "dropping frame for closed connection");
        return;
    };
    match serde_json::to_string(frame) {
        Ok(text) => {
            if conn.tx.send(text).is_err() {
                tracing::debug!(conn = to, "outbound channel closed");
            }
        }
        Err(e) => tracing::warn!(error = %e, "failed to serialize frame"),
    }
}

/// Coarse informational connect timestamp (avoids a chrono dependency).
fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}
