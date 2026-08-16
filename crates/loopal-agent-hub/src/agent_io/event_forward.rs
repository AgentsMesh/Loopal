use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, QualifiedAddress};

use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;

use super::event_admission::agent_may_emit;

pub(super) async fn forward_agent_event(
    hub: &Arc<Mutex<Hub>>,
    connection: &Arc<Connection<Listening>>,
    agent_name: &str,
    params: serde_json::Value,
) -> Result<(), String> {
    let Ok(mut event) = serde_json::from_value::<AgentEvent>(params) else {
        tracing::warn!(agent = %agent_name, "agent/event deserialize failed; dropping");
        return Ok(());
    };
    if !agent_may_emit(&event.payload) {
        tracing::warn!(agent = %agent_name, "agent/event payload is Hub-owned; dropping");
        return Ok(());
    }
    if event.agent_name.is_none() {
        event.agent_name = Some(QualifiedAddress::local(agent_name.to_string()));
    }
    let source_matches = event
        .agent_name
        .as_ref()
        .is_some_and(|address| address.is_local() && address.agent == agent_name);
    if !source_matches {
        tracing::warn!(agent = %agent_name, claimed = ?event.agent_name, "agent/event source mismatch; dropping");
        return Ok(());
    }
    let mut delivery = {
        let mut locked = hub.lock().await;
        let Some(event) = locked
            .registry
            .prepare_connection_event(agent_name, connection, event)
        else {
            tracing::debug!(agent = %agent_name, "stale generation event dropped");
            return Ok(());
        };
        PreparedAuthoritativeEvent::from_hub(&locked, event)
    };
    delivery
        .deliver()
        .await
        .map_err(|error| format!("agent '{agent_name}' event delivery failed: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use loopal_ipc::connection::Connection;
    use loopal_ipc::duplex_pair;
    use loopal_protocol::{AgentEvent, AgentEventPayload};
    use tokio::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn full_event_queue_backpressures_without_holding_hub_lock() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Running))
            .await
            .unwrap();
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let (transport, _peer) = duplex_pair();
        let (connection, _incoming) = Connection::new(transport).into_listening();
        hub.lock()
            .await
            .registry
            .register_connection("main", connection.clone())
            .unwrap();

        let delivery = tokio::spawn({
            let hub = hub.clone();
            let connection = connection.clone();
            async move {
                forward_agent_event(
                    &hub,
                    &connection,
                    "main",
                    serde_json::to_value(AgentEvent::root(AgentEventPayload::AwaitingInput))
                        .unwrap(),
                )
                .await
                .unwrap();
            }
        });
        tokio::task::yield_now().await;
        assert!(!delivery.is_finished());
        drop(
            tokio::time::timeout(Duration::from_millis(100), hub.lock())
                .await
                .expect("backpressured delivery must release the Hub lock"),
        );
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Running
        ));
        tokio::time::timeout(Duration::from_millis(100), delivery)
            .await
            .unwrap()
            .unwrap();
        let delivered = event_rx.recv().await.unwrap();
        assert!(matches!(
            delivered.payload,
            AgentEventPayload::AwaitingInput
        ));
        assert!(delivered.routing_generation.is_some());
    }
}
