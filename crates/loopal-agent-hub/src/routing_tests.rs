use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{AgentEventPayload, MessageSource, QualifiedAddress, UserContent};
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
        let locked = hub.lock().await;
        RouteObservation::from_hub(&locked, "main")
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
    assert!(!route.is_finished());
    assert!(
        tokio::time::timeout(Duration::from_millis(20), agent_rx.recv())
            .await
            .is_err()
    );
    drop(
        tokio::time::timeout(Duration::from_millis(100), hub.lock())
            .await
            .unwrap(),
    );
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
    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
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
        let locked = hub.lock().await;
        RouteObservation::from_hub(&locked, "main")
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
            .is_err()
    );
    tokio::time::timeout(Duration::from_millis(100), shutdown.notified())
        .await
        .unwrap();
}

#[test]
fn peer_message_observation_preserves_hub_authenticated_source() {
    let source = MessageSource::Agent(QualifiedAddress::local("peer"));
    let envelope = Envelope::new(source.clone(), "main", "peer message").with_summary("summary");
    let event = queued_event(&envelope).unwrap();
    let AgentEventPayload::InboxEnqueued {
        source: observed,
        summary,
        ..
    } = event.payload
    else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(observed, source);
    assert_eq!(summary.as_deref(), Some("summary"));
}

#[test]
fn ephemeral_message_observation_does_not_enter_history() {
    for source in [
        MessageSource::Scheduled,
        MessageSource::System("wake".into()),
    ] {
        let envelope = Envelope::new(source, "main", "ephemeral");
        assert!(queued_event(&envelope).is_none());
    }
}
