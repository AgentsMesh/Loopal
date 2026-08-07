//! Child-process event bridge for legacy direct-client integrations.

use tokio::sync::mpsc;
use tracing::info;

use loopal_agent_client::{AgentClient, AgentClientEvent};
use loopal_protocol::{AgentEvent, AgentEventPayload};
use tokio_util::sync::CancellationToken;

/// Track child completion and return the authoritative `agent/completed` result.
/// Used by integration tests; production path uses Hub agent_io_loop.
#[allow(dead_code)]
pub async fn bridge_child_events(
    mut client: AgentClient,
    _parent_tx: &mpsc::Sender<AgentEvent>,
    agent_name: &str,
    cancel_token: &CancellationToken,
) -> Result<String, String> {
    let mut terminal_error = None;
    loop {
        tokio::select! {
            biased;
            () = cancel_token.cancelled() => {
                let _ = client.shutdown().await;
                return Err(format!("sub-agent {agent_name} cancelled"));
            }
            event = client.recv() => match event {
                Some(AgentClientEvent::AgentEvent(ev)) => {
                    if let AgentEventPayload::Error { message } = &ev.payload {
                        terminal_error = Some(message.clone());
                    }
                }
                Some(AgentClientEvent::AgentCompleted(completion)) => {
                    info!(
                        agent = %agent_name,
                        reason = %completion.reason,
                        has_result = completion.result.is_some(),
                        "sub-agent bridge received authoritative completion"
                    );
                    return match completion.reason.as_str() {
                        "goal" => Ok(completion
                            .result
                            .unwrap_or_else(|| "(sub-agent completed)".into())),
                        reason => {
                            let detail = completion.result.or(terminal_error);
                            Err(match detail {
                                Some(detail) => format!(
                                    "sub-agent {agent_name} completed with reason {reason}: {detail}"
                                ),
                                None => format!(
                                    "sub-agent {agent_name} completed with reason {reason} and no result"
                                ),
                            })
                        }
                    };
                }
                None => {
                    return Err(format!(
                        "sub-agent {agent_name} connection closed before agent/completed"
                    ));
                }
            },
        }
    }
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
