//! Server mode — Hub + agent, no TUI.
//!
//! Runs the agent without a terminal UI. Auto-approves all permissions.
//! Used for CI, scripting, and cluster workers.
//!
//! Lifecycle is determined by `--ephemeral` flag (not prompt presence):
//! - `--server` → persistent (waits for input after prompt, if any)
//! - `--server --ephemeral` → exits after agent completes

use std::sync::Arc;

use tracing::info;

use loopal_agent_hub::{HubClient, UiSession};
use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!("starting in server mode (ephemeral={})", cli.ephemeral);

    let ctx = super::hub_bootstrap::bootstrap_hub_and_agent(cli, cwd, config, None).await?;
    let _event_loop = loopal_agent_hub::start_event_loop(ctx.hub.clone(), ctx.event_rx);
    let ui_session = UiSession::connect(ctx.hub.clone(), "server").await;
    info!("server client connected to Hub");

    let output = consume_events(ui_session.event_rx, ui_session.client.clone()).await;

    if !output.is_empty() {
        println!("{output}");
    }

    info!("server mode complete, shutting down");
    let _ = ui_session.client.shutdown_agent().await;
    let _ = ctx.agent_proc.shutdown().await;
    Ok(())
}

/// Consume events, print streaming text, auto-resolve permission/question requests,
/// return final output.
async fn consume_events(
    mut event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    client: Arc<HubClient>,
) -> String {
    let mut last_text = String::new();
    let mut seen_stream = false;

    loop {
        match event_rx.recv().await {
            Ok(event) => {
                let agent_name = event
                    .agent_name
                    .as_ref()
                    .map(|q| q.agent.clone())
                    .unwrap_or_else(|| "main".to_string());
                match event.payload {
                    AgentEventPayload::Stream { text } => {
                        eprint!("{text}");
                        last_text.push_str(&text);
                        seen_stream = true;
                    }
                    AgentEventPayload::ToolPermissionRequest { id, .. } => {
                        info!(agent = %agent_name, tool_call_id = %id, "server: auto-approving permission");
                        client.respond_permission(&agent_name, &id, true).await;
                    }
                    AgentEventPayload::UserQuestionRequest { id, questions } => {
                        info!(agent = %agent_name, question_id = %id, "server: auto-answering question");
                        let answers: Vec<String> = questions
                            .iter()
                            .map(|_| {
                                "Running non-interactively. \
                                 Use your best judgment and proceed. \
                                 Do not wait for user input."
                                    .to_string()
                            })
                            .collect();
                        client.respond_question(&agent_name, &id, answers).await;
                    }
                    AgentEventPayload::AwaitingInput if seen_stream => break,
                    AgentEventPayload::Finished => break,
                    AgentEventPayload::Error { message } => {
                        eprintln!("\nerror: {message}");
                        break;
                    }
                    _ => {}
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "server event consumer lagged");
            }
        }
    }

    eprintln!();
    last_text
}
