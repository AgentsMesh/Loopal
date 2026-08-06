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
use loopal_protocol::{AgentEvent, AgentEventPayload, UiCapabilities};

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!(
        "starting in server mode (ephemeral={})",
        cli.child.ephemeral
    );

    let prepared = super::hub_bootstrap::prepare_hub_and_agent(cli, cwd, config).await?;
    // Install the real responder lease before consuming the typestate that is
    // allowed to start the root agent.
    let capabilities = UiCapabilities {
        permission: true,
        question: true,
        plan_approval: true,
    };
    let ui_session = UiSession::connect(prepared.hub().clone(), "server", capabilities).await;
    info!("server client connected to Hub");
    let ctx = super::hub_bootstrap::start_prepared_hub_and_agent(prepared, cli, cwd, config, None)
        .await?;

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
                    .unwrap_or_else(|| loopal_protocol::ROOT_AGENT_NAME.to_string());
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
                    AgentEventPayload::UserQuestionRequest { id, questions, .. } => {
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
                    AgentEventPayload::PlanApprovalRequest { id, .. } => {
                        info!(agent = %agent_name, request_id = %id, "server: auto-approving plan");
                        client
                            .respond_plan_approval(&agent_name, &id, "approve", None)
                            .await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_ipc::connection::{Connection, Incoming};

    #[tokio::test]
    async fn server_auto_approves_plan_requests() {
        let (client_transport, server_transport) = loopal_ipc::duplex_pair();
        let (client_conn, _client_rx) = Connection::new(client_transport).into_listening();
        let (server_conn, mut server_rx) = Connection::new(server_transport).into_listening();
        let responder = tokio::spawn(async move {
            let Some(Incoming::Request { id, method, params }) = server_rx.recv().await else {
                panic!("expected plan approval response request");
            };
            assert_eq!(
                method,
                loopal_ipc::protocol::methods::HUB_PLAN_APPROVAL_RESPONSE.name
            );
            assert_eq!(params["request_id"], "plan-1");
            assert_eq!(params["decision"], "approve");
            server_conn
                .respond(id, serde_json::json!({"resolved": true}))
                .await
                .unwrap();
        });

        let (event_tx, event_rx) = tokio::sync::broadcast::channel(4);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::PlanApprovalRequest {
                id: "plan-1".into(),
                plan_content: "# Plan".into(),
                plan_path: "/tmp/plan.md".into(),
            }))
            .unwrap();
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Finished))
            .unwrap();

        let client = Arc::new(HubClient::new(client_conn));
        assert!(consume_events(event_rx, client).await.is_empty());
        responder.await.unwrap();
    }
}
