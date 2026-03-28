//! Message routing — point-to-point delivery via Hub.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope};
use tokio::sync::mpsc;

/// Route an envelope to a single target agent.
/// Emits a `MessageRouted` observation event on success.
pub async fn route_to_agent(
    conn: &Arc<Connection>,
    envelope: &Envelope,
    observation_tx: &mpsc::Sender<AgentEvent>,
) -> Result<(), String> {
    let params = serde_json::to_value(envelope)
        .map_err(|e| format!("failed to serialize envelope: {e}"))?;

    conn.send_request(methods::AGENT_MESSAGE.name, params)
        .await
        .map_err(|e| format!("delivery to '{}' failed: {e}", envelope.target))?;

    let event = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: envelope.source.label(),
        target: envelope.target.clone(),
        content_preview: envelope.content_preview().to_string(),
    });
    let _ = observation_tx.try_send(event);
    Ok(())
}
