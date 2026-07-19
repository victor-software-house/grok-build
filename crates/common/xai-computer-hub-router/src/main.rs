//! `grok-hub` — loopback Computer Hub router.
//!
//! ```sh
//! grok-hub --bind 127.0.0.1:10030
//! workspace_server --url ws://127.0.0.1:10030/v1/tools ...
//! workspace-server-probe --url ws://127.0.0.1:10030/v1/tools ...
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use xai_computer_hub_router::{Hub, serve};

#[derive(Parser)]
#[command(name = "grok-hub")]
#[command(about = "Minimal loopback Computer Hub router")]
struct Args {
    /// Address to listen on. Loopback only: auth is dev-grade, and
    /// remote access goes through an SSH tunnel of this port.
    #[arg(long, default_value = "127.0.0.1:10030")]
    bind: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();
    anyhow::ensure!(
        args.bind.ip().is_loopback(),
        "refusing non-loopback bind {}: this router has dev-grade auth; \
         tunnel the loopback port (e.g. ssh -L) for remote access",
        args.bind
    );
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    tracing::info!("grok-hub listening on ws://{}/v1/tools", args.bind);
    serve(listener, Arc::new(Hub::default())).await?;
    Ok(())
}
