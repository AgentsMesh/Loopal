//! Headless mode — Hub + agent, no TUI.
//!
//! Processes the prompt, auto-approves all permissions, prints agent output
//! to stdout, and exits when the agent finishes. Designed for CI, eval
//! harnesses, and scripting.

use tracing::info;

use loopal_agent_hub::UiSession;
use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    if cli.prompt.is_empty() {
        anyhow::bail!("--headless requires a prompt argument");
    }

    info!("starting in headless mode");

    // 1-3. Create Hub + spawn root agent (prompt injected via common)
    let ctx = super::hub_bootstrap::bootstrap_hub_and_agent(cli, cwd, config).await?;

    // 4. Start event broadcast
    let _event_loop = loopal_agent_hub::start_event_loop(ctx.hub.clone(), ctx.event_rx);

    // 5. Connect as headless UI client
    let ui_session = UiSession::connect(ctx.hub.clone(), "headless").await;
    info!("headless client connected to Hub");

    // 6. Auto-approve all permission/question requests in background
    tokio::spawn(auto_approve_relay(
        ui_session.relay_rx,
        ui_session.client.clone(),
    ));

    // 7. Consume events until agent finishes
    let output = consume_events(ui_session.event_rx).await;

    // 8. Print final output
    if !output.is_empty() {
        println!("{output}");
    }

    // 9. Shutdown — close agent's input channel so agent loop exits
    info!("headless mode complete, shutting down");
    let _ = ui_session.client.shutdown_agent().await;
    let _ = ctx.agent_proc.shutdown().await;

    Ok(())
}

/// Consume agent events, print streaming text, return final output.
///
/// In headless mode the agent processes one prompt then enters AwaitingInput.
/// We treat AwaitingInput as "done" since there's no user to provide more input.
async fn consume_events(mut event_rx: tokio::sync::broadcast::Receiver<AgentEvent>) -> String {
    let mut last_text = String::new();
    let mut completion_text: Option<String> = None;
    let mut seen_stream = false;

    loop {
        match event_rx.recv().await {
            Ok(event) => match event.payload {
                AgentEventPayload::Stream { text } => {
                    eprint!("{text}");
                    last_text.push_str(&text);
                    seen_stream = true;
                }
                AgentEventPayload::ToolResult {
                    is_completion: true,
                    result,
                    ..
                } => {
                    completion_text = Some(result);
                }
                AgentEventPayload::AwaitingInput if seen_stream => {
                    // Agent finished processing our prompt and is waiting for
                    // more input. In headless mode there is none — we're done.
                    break;
                }
                AgentEventPayload::Finished | AgentEventPayload::MaxTurnsReached { .. } => {
                    break;
                }
                AgentEventPayload::Error { message } => {
                    eprintln!("\nerror: {message}");
                    break;
                }
                _ => {}
            },
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "headless event consumer lagged");
            }
        }
    }

    eprintln!();
    completion_text.unwrap_or(last_text)
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
                info!(id, "headless: auto-approving permission");
                let _ = client.respond_permission(id, true).await;
            } else if method == loopal_ipc::protocol::methods::AGENT_QUESTION.name {
                info!(id, "headless: auto-approving question");
                let _ = client.respond_question(id, vec!["(auto)".into()]).await;
            }
        }
    }
}
