//! Shared bootstrap logic — creates Hub + spawns root agent.
//!
//! Used by both `multiprocess` (TUI mode) and `acp` (IDE mode) bootstrap paths.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tracing::info;

use loopal_agent_hub::Hub;
use loopal_agent_hub::hub_server;
use loopal_protocol::AgentEvent;

use crate::cli::Cli;

/// Context returned after Hub + root agent bootstrap.
pub struct BootstrapContext {
    pub hub: Arc<Mutex<Hub>>,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub agent_proc: loopal_agent_client::AgentProcess,
}

/// Create Hub, start TCP listener, spawn root agent, register as "main".
pub async fn bootstrap_hub_and_agent(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<BootstrapContext> {
    // 1. Create Hub
    let (event_tx, event_rx) = mpsc::channel(256);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));

    // 2. Start Hub TCP listener for external clients
    let (listener, _hub_port, hub_token) = hub_server::start_hub_listener(hub.clone()).await?;
    let hub_accept = hub.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, hub_token).await;
    });

    // 3. Spawn root agent
    let agent_proc = loopal_agent_client::AgentProcess::spawn(None).await?;
    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    client.initialize().await?;

    let mode_str = if cli.plan { "plan" } else { "act" };
    let prompt = if cli.prompt.is_empty() {
        None
    } else {
        Some(cli.prompt.join(" "))
    };
    client
        .start_agent(
            cwd,
            Some(&config.settings.model),
            Some(mode_str),
            prompt.as_deref(),
            cli.permission.as_deref(),
            cli.no_sandbox,
            cli.resume.as_deref(),
        )
        .await?;

    // Register root agent's stdio as "main" in Hub
    let (root_conn, incoming_rx) = client.into_parts();
    loopal_agent_hub::agent_io::start_agent_io(hub.clone(), "main", root_conn, incoming_rx, true);
    info!("root agent registered as 'main' in Hub");

    Ok(BootstrapContext {
        hub,
        event_rx,
        agent_proc,
    })
}
