//! Message routing — point-to-point delivery via Hub.

use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, MessageSource, QualifiedAddress};

use crate::Hub;
use crate::authoritative_events::AuthoritativeEventSink;

#[derive(Clone)]
pub(crate) struct RouteObservation {
    sink: AuthoritativeEventSink,
    target_generation: Option<u64>,
}

impl RouteObservation {
    pub(crate) fn from_hub(hub: &Hub, target: &str) -> Self {
        Self {
            sink: AuthoritativeEventSink::from_hub(hub),
            target_generation: hub.registry.generation(target),
        }
    }

    async fn deliver_target_event(&self, mut event: AgentEvent) -> Result<(), String> {
        event.routing_generation = self.target_generation;
        self.deliver(event).await
    }

    async fn deliver(&self, event: AgentEvent) -> Result<(), String> {
        self.sink
            .prepare(event)
            .deliver()
            .await
            .map_err(|error| error.to_string())
    }
}

pub async fn route_to_agent(
    conn: &Arc<Connection<Listening>>,
    envelope: &Envelope,
    observation: &RouteObservation,
) -> Result<(), String> {
    if let Some(queued) = queued_event(envelope) {
        observation
            .deliver_target_event(queued)
            .await
            .map_err(|error| {
                format!(
                    "cannot admit routed message for '{}': {error}",
                    envelope.target
                )
            })?;
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
    observation.deliver(routed).await.map_err(|error| {
        format!(
            "message reached '{}' but its routing audit event failed: {error}",
            envelope.target
        )
    })?;
    Ok(())
}

fn queued_event(envelope: &Envelope) -> Option<AgentEvent> {
    let target = QualifiedAddress::local(envelope.target.agent.clone());
    if matches!(envelope.source, MessageSource::Human) {
        return Some(AgentEvent::named(
            target,
            AgentEventPayload::UserMessageQueued {
                envelope_id: envelope.id.to_string(),
                content: envelope.content.text.clone(),
                image_count: envelope.content.images.len(),
                skill_info: envelope.content.skill_info.clone(),
            },
        ));
    }
    if envelope.source.is_ephemeral_in_history() {
        return None;
    }
    Some(AgentEvent::named(
        target,
        AgentEventPayload::InboxEnqueued {
            envelope_id: envelope.id.to_string(),
            source: envelope.source.clone(),
            content: envelope.content.text.clone(),
            summary: envelope.summary.clone(),
        },
    ))
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
