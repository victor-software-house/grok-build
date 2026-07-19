//! Minimal loopback Computer Hub router.
//!
//! The accept side of the wire protocol that `xai-computer-hub-sdk`
//! clients already speak: tool servers (`workspace_server`,
//! `?role=tool_server`) and harnesses (`ToolHarness` /
//! `WorkspaceClient`, `?role=harness`) dial `ws://<bind>/v1/tools`,
//! perform the `hello`/`hello_ack` handshake, and the router relays
//! session binds, tool calls, progress frames, and hooks between them.
//!
//! Scope: loopback / trusted-tunnel deployments. Auth is dev-grade
//! (unsigned JWT `sub` extraction); do not expose this listener on a
//! non-loopback interface.

#![forbid(unsafe_code)]

mod auth;
mod hub;
mod ws;

pub use hub::Hub;
pub use ws::{app, serve};
