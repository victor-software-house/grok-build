//! Axum WebSocket endpoint and per-connection actor.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt as _, StreamExt as _};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use xai_tool_protocol::{HelloMsg, UserId};

use crate::Hub;
use crate::auth::user_from_headers;

struct AppState {
    hub: Arc<Hub>,
    next_conn: AtomicU64,
}

/// Build the axum app serving `/v1/tools`.
pub fn app(hub: Arc<Hub>) -> Router {
    let state = Arc::new(AppState {
        hub,
        next_conn: AtomicU64::new(1),
    });
    Router::new()
        .route("/v1/tools", get(upgrade))
        .with_state(state)
}

/// Serve until the listener errors or the task is aborted.
pub async fn serve(listener: TcpListener, hub: Arc<Hub>) -> std::io::Result<()> {
    axum::serve(
        listener,
        app(hub).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

async fn upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user = user_from_headers(&headers);
    let key = state.next_conn.fetch_add(1, Ordering::Relaxed);
    tracing::info!(conn = key, %peer, user = user.as_str(), "upgrading connection");
    ws.on_upgrade(move |socket| connection(socket, state, key, user))
}

async fn connection(socket: WebSocket, state: Arc<AppState>, key: u64, user: UserId) {
    let (mut sink, mut stream) = socket.split();

    // ── Handshake: first text frame must be a HelloMsg. ──────────────
    let hello = loop {
        match stream.next().await {
            Some(Ok(Message::Text(text))) => {
                match serde_json::from_str::<HelloMsg>(&text) {
                    Ok(hello) => break hello,
                    Err(e) => {
                        tracing::warn!(conn = key, error = %e, "invalid hello frame");
                        return;
                    }
                }
            }
            // Axum answers protocol-level pings transparently.
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
            _ => return,
        }
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let ack = match state.hub.register(key, user, &hello, tx) {
        Ok(ack) => ack,
        Err(reason) => {
            tracing::warn!(conn = key, reason, "rejecting connection");
            return;
        }
    };
    let ack_text = match serde_json::to_string(&ack) {
        Ok(text) => text,
        Err(e) => {
            tracing::error!(conn = key, error = %e, "hello_ack serialization failed");
            state.hub.disconnect(key);
            return;
        }
    };
    if sink.send(Message::Text(ack_text.into())).await.is_err() {
        state.hub.disconnect(key);
        return;
    }

    // ── Writer: drain the outbound queue into the sink. ──────────────
    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // ── Reader: route every inbound text frame. ──────────────────────
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(Message::Text(text)) => state.hub.route(key, &text),
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {} // binary/ping/pong: nothing to do
        }
    }

    state.hub.disconnect(key);
    writer.abort();
}
