//! Hub-first bootstrap — Hub starts, spawns root agent, runs headless.
//!
//! Used by `loopal --hub` for headless mode (no TUI).

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::hub_server;

use crate::cli::Cli;

/// Run the Hub + root agent in headless mode (no TUI).
pub async fn run_hub(
    cli: &Cli,
    cwd: &std::path::Path,
    _config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!("starting Hub in headless mode");

    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(256);
    let hub = Arc::new(Mutex::new(AgentHub::new(event_tx)));

    // Start Hub TCP listener
    let (listener, hub_port, token) = hub_server::start_hub_listener(hub.clone()).await?;

    // Write Hub server info for discovery
    loopal_agent_server::server_info::write_server_info(hub_port, &token)?;

    // Accept connections in background
    let hub_accept = hub.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, token).await;
    });

    // Spawn root agent
    let root_proc = loopal_agent_client::AgentProcess::spawn_with_args(
        None,
        &["--hub-port", &hub_port.to_string()],
    )
    .await?;
    let _ = cwd; // cwd passed via agent/start from external clients
    let _ = cli; // prompt/model handled by connecting clients
    info!("root agent spawned, Hub running headless on port {hub_port}");

    // Wait for root agent to exit (Hub runs until agent stops)
    let _ = root_proc.wait().await;

    loopal_agent_server::server_info::remove_server_info();
    Ok(())
}
