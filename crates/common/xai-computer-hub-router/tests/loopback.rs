//! Loopback end-to-end: the REAL SDK ends (`ToolServer` + `ToolHarness`)
//! against an in-process router.
//!
//! Covers the TODO acceptance list: handshake for both roles, streamed
//! progress (bash-like), write and read as independent RPCs, concurrent
//! call correlation, and disconnect cleanup.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use xai_computer_hub_router::{Hub, serve};
use xai_computer_hub_sdk::pool::HubConnectionPool;
use xai_computer_hub_sdk::{
    AuthCredential, ToolHarness, ToolHarnessBuilder, ToolServer, ToolServerBuilder,
    ToolServerHandler,
};
use xai_tool_protocol::{ServerId, SessionId};
use xai_tool_runtime::{
    ToolCallContext, ToolId, ToolProgress, ToolStream, ToolStreamItem, TypedToolOutput,
    terminal_only, with_progress,
};
use xai_tool_types::ToolDescription;

const USER: &str = "loopback-tester";
const SERVER_ID: &str = "srv-loopback";

/// Unsigned JWT with a `sub` claim — the local-dev credential shape the
/// probe uses against the local-auth-dev hub.
fn dev_bearer(user_id: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"sub":"{user_id}"}}"#).as_bytes());
    format!("{header}.{payload}.")
}

fn credential() -> AuthCredential {
    AuthCredential::Bearer {
        token: dev_bearer(USER),
    }
}

fn output(tool: &str, value: Value) -> TypedToolOutput {
    TypedToolOutput {
        tool_id: ToolId::new(tool).expect("valid tool id"),
        value: value.clone(),
        model_output: vec![xai_tool_runtime::ContentBlock::Text {
            text: value.to_string(),
        }],
        chat_completion_output: None,
    }
}

/// Echoes its arguments after two streamed progress chunks.
struct EchoTool;

#[async_trait]
impl ToolServerHandler for EchoTool {
    fn tool_id(&self) -> ToolId {
        ToolId::new("echo").expect("valid tool id")
    }
    fn description(&self) -> ToolDescription {
        ToolDescription::new("echo", "echo arguments after streaming progress")
    }
    async fn handle_call(&self, _ctx: ToolCallContext, args: Value) -> ToolStream<TypedToolOutput> {
        let progress = futures::stream::iter(vec![
            ToolProgress::Text {
                text: "chunk-1".to_owned(),
            },
            ToolProgress::Text {
                text: "chunk-2".to_owned(),
            },
        ]);
        with_progress(progress, async move { Ok(output("echo", json!({ "echo": args }))) })
    }
}

/// Writes `content` to `path` under its root.
struct WriteTool {
    root: PathBuf,
}

#[async_trait]
impl ToolServerHandler for WriteTool {
    fn tool_id(&self) -> ToolId {
        ToolId::new("fs_write").expect("valid tool id")
    }
    fn description(&self) -> ToolDescription {
        ToolDescription::new("fs_write", "write a file below the test root")
    }
    async fn handle_call(&self, _ctx: ToolCallContext, args: Value) -> ToolStream<TypedToolOutput> {
        let path = self.root.join(args["path"].as_str().unwrap_or_default());
        let content = args["content"].as_str().unwrap_or_default().to_owned();
        let written = std::fs::write(&path, &content).is_ok();
        terminal_only(Ok(output(
            "fs_write",
            json!({ "written": written, "path": path.display().to_string() }),
        )))
    }
}

/// Reads `path` under its root.
struct ReadTool {
    root: PathBuf,
}

#[async_trait]
impl ToolServerHandler for ReadTool {
    fn tool_id(&self) -> ToolId {
        ToolId::new("fs_read").expect("valid tool id")
    }
    fn description(&self) -> ToolDescription {
        ToolDescription::new("fs_read", "read a file below the test root")
    }
    async fn handle_call(&self, _ctx: ToolCallContext, args: Value) -> ToolStream<TypedToolOutput> {
        let path = self.root.join(args["path"].as_str().unwrap_or_default());
        let content = std::fs::read_to_string(&path).ok();
        terminal_only(Ok(output(
            "fs_read",
            json!({ "found": content.is_some(), "content": content }),
        )))
    }
}

struct Stack {
    server: ToolServer,
    harness: ToolHarness,
    /// The server's connection pool. Unshared pools have no idle
    /// reaper: closing the server's socket requires dropping every
    /// ToolServer clone AND calling `sweep_idle` here (pool.rs:99-101).
    server_pool: Arc<HubConnectionPool>,
    _router: tokio::task::JoinHandle<()>,
    _server_loop: tokio::task::JoinHandle<()>,
}

/// Hard per-test deadline so a wedged wire flow can never hang the CI
/// job; `cargo test` has no timeout of its own.
async fn deadline<F: Future<Output = ()>>(fut: F) {
    tokio::time::timeout(std::time::Duration::from_secs(120), fut)
        .await
        .expect("test exceeded its 120s deadline");
}

async fn start_stack(root: PathBuf) -> Stack {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral loopback port");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    let hub = Arc::new(Hub::default());
    let router = tokio::spawn(async move {
        let _ = serve(listener, hub).await;
    });
    let url = url::Url::parse(&format!("ws://{addr}/v1/tools")).expect("valid url");

    let server_pool = HubConnectionPool::new();
    let server = ToolServerBuilder::default()
        .pool(server_pool.clone())
        .url(url.clone())
        .auth(credential())
        .server_id(ServerId::new(SERVER_ID).expect("valid server id"))
        .server_description("loopback test server")
        .tool(EchoTool)
        .tool(WriteTool { root: root.clone() })
        .tool(ReadTool { root })
        .build()
        .await
        .expect("tool server connects and handshakes");
    let server_for_loop = server.clone();
    let server_loop = tokio::spawn(async move {
        let _ = server_for_loop.run().await;
    });

    let session = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4())).expect("valid session");
    let harness = ToolHarnessBuilder::default()
        .pool(HubConnectionPool::new())
        .url(url)
        .auth(credential())
        .session(session)
        .build()
        .await
        .expect("harness connects and handshakes");

    Stack {
        server,
        harness,
        server_pool,
        _router: router,
        _server_loop: server_loop,
    }
}

async fn bind(stack: &Stack) -> Vec<ToolDescription> {
    stack
        .harness
        .session_bind(SERVER_ID, None, None)
        .await
        .expect("session_bind_server succeeds")
}

async fn call(
    harness: &ToolHarness,
    tool: &str,
    args: Value,
) -> (usize, Result<Value, String>) {
    let mut stream = harness
        .call(
            ToolId::new(tool).expect("valid tool id"),
            args,
            ToolCallContext::default(),
        )
        .await;
    let mut progress = 0usize;
    loop {
        let item = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await;
        match item {
            Some(ToolStreamItem::Progress(_)) => progress += 1,
            Some(ToolStreamItem::Terminal(Ok(typed))) => return (progress, Ok(typed.value)),
            Some(ToolStreamItem::Terminal(Err(e))) => return (progress, Err(e.to_string())),
            None => return (progress, Err("stream ended without terminal".to_owned())),
        }
    }
}

fn test_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hub-router-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create test root");
    dir
}

#[tokio::test]
async fn discovery_bind_and_streamed_echo() {
    deadline(async {
        let stack = start_stack(test_root("echo")).await;

        // Discovery: the server registered under this user is visible.
        let servers = stack.harness.list_servers().await.expect("servers.list");
        assert!(
            servers.iter().any(|s| s.server_id.as_str() == SERVER_ID),
            "expected {SERVER_ID} in servers.list, got {servers:?}"
        );

        // Bind returns the tool snapshot from the server's bind response.
        let tools = bind(&stack).await;
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"echo"), "echo missing from {names:?}");

        // Streamed call: progress chunks arrive before the terminal value.
        let (progress, result) = call(&stack.harness, "echo", json!({ "nonce": "n-1" })).await;
        let value = result.expect("echo terminal value");
        assert_eq!(value["echo"]["nonce"], "n-1");
        assert!(progress >= 2, "expected streamed progress, got {progress}");
    })
    .await;
}

#[tokio::test]
async fn write_then_read_roundtrip() {
    deadline(async {
        let stack = start_stack(test_root("rw")).await;
        bind(&stack).await;

        let (_, written) = call(
            &stack.harness,
            "fs_write",
            json!({ "path": "note.txt", "content": "hello-roundtrip" }),
        )
        .await;
        assert_eq!(written.expect("fs_write result")["written"], true);

        let (_, read) = call(&stack.harness, "fs_read", json!({ "path": "note.txt" })).await;
        let read = read.expect("fs_read result");
        assert_eq!(read["found"], true);
        assert_eq!(read["content"], "hello-roundtrip");
    })
    .await;
}

#[tokio::test]
async fn concurrent_calls_correlate_independently() {
    deadline(async {
        let stack = start_stack(test_root("concurrent")).await;
        bind(&stack).await;

        let a = call(&stack.harness, "echo", json!({ "nonce": "left" }));
        let b = call(&stack.harness, "echo", json!({ "nonce": "right" }));
        let ((_, a), (_, b)) = tokio::join!(a, b);
        assert_eq!(a.expect("left result")["echo"]["nonce"], "left");
        assert_eq!(b.expect("right result")["echo"]["nonce"], "right");
    })
    .await;
}

#[tokio::test]
async fn server_disconnect_cleans_registry_and_fails_rebind() {
    // Deliberately does NOT issue a tool call after the disconnect: the
    // SDK treats "workspace gone" (-32005) on an in-flight call as
    // retryable and parks the caller, which would hang the test. The
    // router-owned cleanup contract is asserted instead: once the
    // server's socket closes, it leaves discovery and a fresh bind
    // fails fast.
    deadline(async {
        let Stack {
            server,
            harness,
            server_pool,
            _router,
            _server_loop,
        } = start_stack(test_root("shutdown")).await;
        harness
            .session_bind(SERVER_ID, None, None)
            .await
            .expect("session_bind_server succeeds");

        // Teardown order matters: shutdown() cancels the borrow token
        // (server.rs:1731) which makes run() return AND abort its
        // clone-holding notif/reconnect/session tasks (1607-1611);
        // aborting the run task instead would leak those clones. Only
        // after run() returns and the last ToolServer drops does the
        // pool hold the sole Arc — then one zero-TTL sweep evicts it
        // and its Drop closes the socket (unshared pools never reap on
        // their own).
        server.shutdown().await.expect("clean shutdown");
        let _ = _server_loop.await;
        drop(server);
        let evicted = server_pool.sweep_idle(std::time::Duration::ZERO);
        assert_eq!(evicted, 1, "teardown did not release the pooled connection");

        // The router observes the socket close asynchronously; poll
        // discovery until the registration is gone.
        for _ in 0..100 {
            let servers = harness.list_servers().await.expect("servers.list");
            if !servers.iter().any(|s| s.server_id.as_str() == SERVER_ID) {
                let rebind = harness.session_bind(SERVER_ID, None, None).await;
                assert!(
                    rebind.is_err(),
                    "bind to a disconnected server must fail, got {rebind:?}"
                );
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("server still listed 10s after socket close");
    })
    .await;
}
