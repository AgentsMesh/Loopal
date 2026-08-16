//! Server mode — Hub + agent, no TUI.
//!
//! Runs the agent without a terminal UI. Auto-approves all permissions.
//! Used for CI, scripting, and cluster workers.
//!
//! Lifecycle is determined by `--ephemeral` flag (not prompt presence):
//! - `--server` → persistent (waits for input after prompt, if any)
//! - `--server --ephemeral` → exits after agent completes

use std::sync::Arc;

use loopal_agent_hub::HubClient;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use tracing::info;

#[path = "server_mode/lifecycle.rs"]
mod lifecycle;

pub use lifecycle::run;

const NON_INTERACTIVE_ANSWER: &str = "Running non-interactively. \
Use your best judgment and proceed. \
Do not wait for user input.";

async fn consume_events(
    mut event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    client: Arc<HubClient>,
) -> String {
    let mut last_text = String::new();

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
                    }
                    AgentEventPayload::ToolPermissionRequest {
                        id,
                        permission_intent,
                        ..
                    } => {
                        info!(agent = %agent_name, tool_call_id = %id, "server: auto-approving permission");
                        let digest = permission_intent
                            .as_deref()
                            .map(loopal_protocol::PermissionIntent::intent_digest);
                        client
                            .respond_permission(&agent_name, &id, digest, true)
                            .await;
                    }
                    AgentEventPayload::UserQuestionRequest { id, questions, .. } => {
                        info!(agent = %agent_name, question_id = %id, "server: auto-answering question");
                        let answers = questions
                            .iter()
                            .map(|_| NON_INTERACTIVE_ANSWER.to_string())
                            .collect();
                        client.respond_question(&agent_name, &id, answers).await;
                    }
                    AgentEventPayload::PlanApprovalRequest { id, .. } => {
                        info!(agent = %agent_name, request_id = %id, "server: auto-approving plan");
                        client
                            .respond_plan_approval(&agent_name, &id, "approve", None)
                            .await;
                    }
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
#[path = "server_mode/tests.rs"]
mod tests;
