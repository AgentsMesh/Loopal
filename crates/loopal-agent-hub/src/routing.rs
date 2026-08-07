//! Message routing — point-to-point delivery via Hub.

use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, MessageSource, QualifiedAddress};

use crate::Hub;
use crate::authoritative_events::AuthoritativeEventSink;

/// Generation-bound observation channel captured while resolving a route.
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
    conn: &Arc<Connection<Listening>>,
    envelope: &Envelope,
    observation: &RouteObservation,
) -> Result<(), String> {
    if matches!(envelope.source, MessageSource::Human) {
        let queued = AgentEvent::named(
            QualifiedAddress::local(envelope.target.agent.clone()),
            AgentEventPayload::UserMessageQueued {
                envelope_id: envelope.id.to_string(),
                content: envelope.content.text.clone(),
                image_count: envelope.content.images.len(),
                skill_info: envelope.content.skill_info.clone(),
            },
        );
        observation
            .deliver_target_event(queued)
            .await
            .map_err(|error| {
                format!(
                    "cannot admit user message for '{}': {error}",
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use loopal_ipc::connection::Incoming;
    use loopal_protocol::{AgentEventPayload, UserContent};
    use tokio::sync::{Mutex, mpsc};

    use super::*;

    #[tokio::test]
    async fn full_observation_queue_backpressures_and_preserves_route_order() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Running))
            .await
            .unwrap();
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
        let (agent, mut agent_rx) = Connection::new(agent_transport).into_listening();
        let (hub_connection, _hub_rx) = Connection::new(hub_transport).into_listening();
        hub.lock()
            .await
            .registry
            .register_connection("main", hub_connection.clone())
            .unwrap();
        let observation = {
            let hub = hub.lock().await;
            RouteObservation::from_hub(&hub, "main")
        };
        let envelope = Envelope::new(
            MessageSource::Human,
            QualifiedAddress::local("main"),
            UserContent::from("queued under pressure"),
        );

        let route = tokio::spawn({
            let hub_connection = hub_connection.clone();
            let observation = observation.clone();
            async move { route_to_agent(&hub_connection, &envelope, &observation).await }
        });
        tokio::task::yield_now().await;
        assert!(
            !route.is_finished(),
            "a full queue must backpressure routing"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), agent_rx.recv())
                .await
                .is_err(),
            "the agent/message RPC must not overtake UserMessageQueued admission"
        );
        let guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
            .await
            .expect("route backpressure must not hold the Hub lock");
        drop(guard);

        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Running
        ));
        let queued = event_rx.recv().await.unwrap();
        assert!(matches!(
            queued.payload,
            AgentEventPayload::UserMessageQueued { .. }
        ));
        assert!(queued.routing_generation.is_some());

        let Incoming::Request { id, method, .. } = agent_rx.recv().await.unwrap() else {
            panic!("expected agent/message request");
        };
        assert_eq!(method, methods::AGENT_MESSAGE.name);
        agent
            .respond(id, serde_json::json!({"ok": true}))
            .await
            .unwrap();
        let routed = event_rx.recv().await.unwrap();
        assert!(matches!(
            routed.payload,
            AgentEventPayload::MessageRouted { .. }
        ));
        route.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn closed_observation_queue_rejects_route_before_agent_delivery() {
        let (event_tx, event_rx) = mpsc::channel(1);
        drop(event_rx);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let shutdown = hub.lock().await.shutdown_signal.clone();
        let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
        let (_agent, mut agent_rx) = Connection::new(agent_transport).into_listening();
        let (hub_connection, _hub_rx) = Connection::new(hub_transport).into_listening();
        hub.lock()
            .await
            .registry
            .register_connection("main", hub_connection.clone())
            .unwrap();
        let observation = {
            let hub = hub.lock().await;
            RouteObservation::from_hub(&hub, "main")
        };
        let envelope = Envelope::new(
            MessageSource::Human,
            QualifiedAddress::local("main"),
            UserContent::from("must not disappear"),
        );

        let error = route_to_agent(&hub_connection, &envelope, &observation)
            .await
            .unwrap_err();
        assert!(error.contains("authoritative Hub event queue closed"));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), agent_rx.recv())
                .await
                .is_err(),
            "routing must fail before agent delivery when the observation sink is closed"
        );
        tokio::time::timeout(Duration::from_millis(100), shutdown.notified())
            .await
            .expect("closed route sink must invalidate the Hub");
    }
}
