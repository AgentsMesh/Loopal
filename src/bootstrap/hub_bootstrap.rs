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
    /// Root agent's session ID (for sub-agent ref persistence).
    pub root_session_id: String,
    /// TCP listener token — printed on stderr so external clients can
    /// `--attach-hub` this Hub.
    pub hub_token: String,
}

/// Create Hub, start TCP listener, spawn root agent, register as "main".
pub async fn bootstrap_hub_and_agent(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<BootstrapContext> {
    let (event_tx, event_rx) = mpsc::channel(256);
    let hub = Arc::new(Mutex::new(Hub::with_cwd(event_tx, cwd.to_path_buf())));
    hub.lock().await.max_total_agents = config.settings.harness.agent_max_total;

    let (listener, port, hub_token) = hub_server::start_hub_listener(hub.clone()).await?;
    {
        let mut h = hub.lock().await;
        h.listener_port = Some(port);
        h.listener_token = Some(hub_token.clone());
    }
    let hub_accept = hub.clone();
    let token_for_loop = hub_token.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, token_for_loop).await;
    });

    if let Some(ref meta_addr) = cli.join_hub {
        super::uplink_bootstrap::connect_to_meta_hub(&hub, meta_addr, cli.hub_name.as_deref())
            .await?;
    }

    let agent_proc = loopal_agent_client::AgentProcess::spawn(None).await?;
    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    client.initialize().await?;

    let mode_str = if cli.plan { "plan" } else { "act" };
    let prompt = if cli.prompt.is_empty() {
        None
    } else {
        Some(cli.prompt.join(" "))
    };
    let lifecycle_str = if cli.ephemeral {
        Some("ephemeral")
    } else {
        None // default: persistent (server decides based on prompt)
    };
    let root_session_id = client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: cwd.to_path_buf(),
            model: Some(config.settings.model.clone()),
            mode: Some(mode_str.to_string()),
            prompt: prompt.clone(),
            permission_mode: cli.permission.clone(),
            no_sandbox: cli.no_sandbox,
            resume: resume.map(String::from),
            lifecycle: lifecycle_str.map(String::from),
            agent_type: None,
            depth: None,
            fork_context: None,
        })
        .await?;

    let (root_conn, incoming_rx) = client.into_parts();
    loopal_agent_hub::agent_io::start_agent_io(hub.clone(), "main", root_conn, incoming_rx);
    info!("root agent registered as 'main' in Hub");

    Ok(BootstrapContext {
        hub,
        event_rx,
        agent_proc,
        root_session_id,
        hub_token,
    })
}
