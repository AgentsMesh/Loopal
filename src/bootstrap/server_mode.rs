//! Server mode — Hub + agent, no TUI.
//!
//! Runs the agent without a terminal UI. Auto-approves all permissions.
//! Used for CI, scripting, and cluster workers.
//!
//! Lifecycle is determined by `--ephemeral` flag (not prompt presence):
//! - `--server` → persistent (waits for input after prompt, if any)
//! - `--server --ephemeral` → exits after agent completes

use tracing::info;

use loopal_agent_hub::UiSession;
use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!("starting in server mode (ephemeral={})", cli.ephemeral);

    let ctx = super::hub_bootstrap::bootstrap_hub_and_agent(cli, cwd, config).await?;
    let _event_loop = loopal_agent_hub::start_event_loop(ctx.hub.clone(), ctx.event_rx);
    let ui_session = UiSession::connect(ctx.hub.clone(), "server").await;
    info!("server client connected to Hub");

    tokio::spawn(auto_approve_relay(
        ui_session.relay_rx,
        ui_session.client.clone(),
    ));

    let output = consume_events(ui_session.event_rx).await;

    if !output.is_empty() {
        println!("{output}");
    }

    info!("server mode complete, shutting down");
    let _ = ui_session.client.shutdown_agent().await;
    let _ = ctx.agent_proc.shutdown().await;
    Ok(())
}

/// Consume events, print streaming text, return final output.
async fn consume_events(mut event_rx: tokio::sync::broadcast::Receiver<AgentEvent>) -> String {
    let mut last_text = String::new();
    let mut seen_stream = false;

    loop {
        match event_rx.recv().await {
            Ok(event) => match event.payload {
                AgentEventPayload::Stream { text } => {
                    eprint!("{text}");
                    last_text.push_str(&text);
                    seen_stream = true;
                }
                AgentEventPayload::AwaitingInput if seen_stream => break,
                AgentEventPayload::Finished => break,
                AgentEventPayload::Error { message } => {
                    eprintln!("\nerror: {message}");
                    break;
                }
                _ => {}
            },
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "server event consumer lagged");
            }
        }
    }

    eprintln!();
    last_text
}

/// Auto-approve all permission and question relay requests.
async fn auto_approve_relay(
    mut rx: tokio::sync::mpsc::Receiver<loopal_ipc::connection::Incoming>,
    client: std::sync::Arc<loopal_agent_hub::HubClient>,
) {
    use loopal_ipc::connection::Incoming;

    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, method, .. } = msg {
            if method == loopal_ipc::protocol::methods::AGENT_PERMISSION.name {
                info!(id, "server: auto-approving permission");
                let _ = client.respond_permission(id, true).await;
            } else if method == loopal_ipc::protocol::methods::AGENT_QUESTION.name {
                info!(id, "server: auto-approving question");
                let _ = client.respond_question(id, vec!["(auto)".into()]).await;
            }
        }
    }
}
