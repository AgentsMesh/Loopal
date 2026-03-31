//! Child-process event bridge: track sub-agent completion and collect result text.

use tokio::sync::mpsc;
use tracing::info;

use loopal_agent_client::{AgentClient, AgentClientEvent};
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use tokio_util::sync::CancellationToken;

/// Track child completion and collect result text.
/// Used by integration tests; production path uses Hub agent_io_loop.
#[allow(dead_code)]
pub async fn bridge_child_events(
    mut client: AgentClient,
    _parent_tx: &mpsc::Sender<AgentEvent>,
    agent_name: &str,
    cancel_token: &CancellationToken,
) -> Result<String, String> {
    let mut stream_text = String::new();
    let mut completion_result: Option<String> = None;
    loop {
        tokio::select! {
            event = client.recv() => match event {
                Some(AgentClientEvent::AgentEvent(ev)) => {
                    match &ev.payload {
                        AgentEventPayload::Stream { text } => {
                            stream_text.push_str(text);
                        }
                        // Capture AttemptCompletion result — this is the
                        // sub-agent's primary output, not the Stream text.
                        AgentEventPayload::ToolResult { result, is_completion: true, .. } =>
                        {
                            completion_result = Some(result.clone());
                        }
                        // Session finished — child server will exit on its own
                        // for prompt-driven sessions. Just break.
                        AgentEventPayload::Finished => {
                            break;
                        }
                        _ => {}
                    }
                }
                Some(AgentClientEvent::PermissionRequest { id, .. }) => {
                    let _ = client.respond_permission(id, false).await;
                }
                Some(AgentClientEvent::QuestionRequest { id, .. }) => {
                    let resp = UserQuestionResponse {
                        answers: vec!["(sub-agent: auto-cancelled)".into()],
                    };
                    let _ = client.respond_question(id, &resp).await;
                }
                None => break,
            },
            () = cancel_token.cancelled() => {
                let _ = client.shutdown().await;
                break;
            }
        }
    }
    info!(agent = %agent_name, "sub-agent bridge ended");
    // Prefer AttemptCompletion result over accumulated stream text.
    // The content is already clean (no prefix) since the producer uses
    // ToolResult::completion() which sets is_completion: true.
    let output = completion_result.unwrap_or_else(|| {
        if stream_text.is_empty() {
            "(sub-agent completed)".into()
        } else {
            stream_text
        }
    });
    Ok(output)
}

/// Read child's TCP server_info (port, token) — legacy, kept for tests.
#[allow(dead_code)]
pub(crate) fn read_child_server_info(pid: u32) -> Option<(u16, String)> {
    let path = loopal_config::locations::volatile_dir()
        .join("run")
        .join(format!("{pid}.json"));
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let v: serde_json::Value = serde_json::from_str(&content).ok()?;
            let port = v["port"].as_u64()? as u16;
            let token = v["token"].as_str()?.to_string();
            info!(pid, port, path = %path.display(), "read child server_info");
            Some((port, token))
        }
        Err(e) => {
            tracing::warn!(pid, path = %path.display(), error = %e, "failed to read child server_info");
            None
        }
    }
}
