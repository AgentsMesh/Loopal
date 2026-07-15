//! MetaHub bootstrap — starts MetaHub as a standalone cluster coordinator.
//!
//! Usage: `loopal --meta-hub 0.0.0.0:9900`
//!
//! The MetaHub listens for Sub-Hub connections, coordinates cross-hub
//! communication, and aggregates events from all connected Hubs.

use std::sync::Arc;

use std::io::Write;
use tokio::sync::Mutex;
use tracing::info;

use loopal_meta_hub::MetaHub;
use loopal_meta_hub::server;

/// Run MetaHub as a standalone coordinator process.
///
/// Blocks forever (until SIGINT/SIGTERM), accepting Sub-Hub connections.
pub async fn run(bind_addr: &str, parent_pid: Option<u32>) -> anyhow::Result<()> {
    if let Some(pid) = parent_pid {
        super::parent_liveness::validate(pid)?;
    }
    info!("starting MetaHub on {bind_addr}");

    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));

    let (listener, token) = server::start_meta_listener(bind_addr).await?;
    let local_addr = listener.local_addr()?;

    if let Some(pid) = parent_pid {
        println!(
            "LOOPAL_METAHUB {}",
            serde_json::json!({
                "protocol_version": 1,
                "phase": "ready",
                "address": local_addr.to_string(),
                "token": token,
                "pid": std::process::id(),
                "parent_pid": pid,
            })
        );
        std::io::stdout().flush()?;
        tokio::select! {
            () = server::meta_accept_loop(listener, meta_hub, token) => {},
            () = super::parent_liveness::wait_until_exit(pid) => {},
        }
    } else {
        eprintln!("MetaHub listening on {local_addr}");
        eprintln!("Token: {token}");
        eprintln!();
        eprintln!("Connect a Loopal instance with:");
        eprintln!("  LOOPAL_META_HUB_TOKEN={token} loopal --join-hub {local_addr}");
        server::meta_accept_loop(listener, meta_hub, token).await;
    }

    Ok(())
}
