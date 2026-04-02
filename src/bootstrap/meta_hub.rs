//! MetaHub bootstrap — starts MetaHub as a standalone cluster coordinator.
//!
//! Usage: `loopal --meta-hub 0.0.0.0:9900`
//!
//! The MetaHub listens for Sub-Hub connections, coordinates cross-hub
//! communication, and aggregates events from all connected Hubs.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use loopal_meta_hub::MetaHub;
use loopal_meta_hub::server;

/// Run MetaHub as a standalone coordinator process.
///
/// Blocks forever (until SIGINT/SIGTERM), accepting Sub-Hub connections.
pub async fn run(bind_addr: &str) -> anyhow::Result<()> {
    info!("starting MetaHub on {bind_addr}");

    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));

    let (listener, token) = server::start_meta_listener(bind_addr).await?;
    let local_addr = listener.local_addr()?;

    // Print connection info for Sub-Hubs to use
    eprintln!("MetaHub listening on {local_addr}");
    eprintln!("Token: {token}");
    eprintln!();
    eprintln!("Connect a Loopal instance with:");
    eprintln!("  LOOPAL_META_HUB_TOKEN={token} loopal --join-hub {local_addr}");

    // Accept loop runs forever
    server::meta_accept_loop(listener, meta_hub, token).await;

    Ok(())
}
