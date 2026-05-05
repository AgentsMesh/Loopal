//! Message routing — point-to-point delivery via Hub.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, MessageSource, QualifiedAddress};
use tokio::sync::mpsc;

/// Route an envelope to a single target agent.
///
/// Order matters: emit `UserMessageQueued` BEFORE `send_request` so all
/// UI clients see the user's row land in the conversation before the
/// agent's reply events (which agents typically emit during request
/// processing, before the request response returns).
///
/// On successful delivery a `MessageRouted` audit event is emitted
/// after the response returns.
pub async fn route_to_agent(
    conn: &Arc<Connection>,
    envelope: &Envelope,
    observation_tx: &mpsc::Sender<AgentEvent>,
) -> Result<(), String> {
    if matches!(envelope.source, MessageSource::Human) {
        let queued = AgentEvent::named(
            QualifiedAddress::local(envelope.target.agent.clone()),
            AgentEventPayload::UserMessageQueued {
                message_id: envelope.id.to_string(),
                content: envelope.content.text.clone(),
                image_count: envelope.content.images.len(),
            },
        );
        if observation_tx.try_send(queued).is_err() {
            tracing::warn!(
                target = %envelope.target,
                "UserMessageQueued dropped (channel full)"
            );
        }
    }

    let params =
        serde_json::to_value(envelope).map_err(|e| format!("failed to serialize envelope: {e}"))?;

    conn.send_request(methods::AGENT_MESSAGE.name, params)
        .await
        .map_err(|e| format!("delivery to '{}' failed: {e}", envelope.target))?;

    let routed = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: envelope.source.clone(),
        target: envelope.target.clone(),
        content_preview: envelope.content_preview().to_string(),
    });
    if observation_tx.try_send(routed).is_err() {
        tracing::warn!(
            target = %envelope.target,
            "MessageRouted dropped (channel full)"
        );
    }
    Ok(())
}
