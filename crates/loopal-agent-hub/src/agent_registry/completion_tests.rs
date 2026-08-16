use std::time::Duration;

use loopal_ipc::connection::Connection;

use super::*;

#[tokio::test]
async fn cancelled_backpressure_retains_the_ordered_terminal_sequence() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();
    let mut registry = AgentRegistry::new(event_tx);
    let mut pending = registry.emit_agent_completion(
        "worker",
        AgentCompletion::new("error", Some("provider failed".into())),
    );

    assert!(
        tokio::time::timeout(Duration::from_millis(10), pending.deliver_events())
            .await
            .is_err(),
        "full event queue should backpressure completion delivery"
    );
    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Running
    ));

    let delivery = pending.deliver_events();
    let receive = async {
        let error = event_rx.recv().await.unwrap();
        let finished = event_rx.recv().await.unwrap();
        (error, finished)
    };
    let (result, (error, finished)) = tokio::join!(delivery, receive);
    result.unwrap();
    assert!(matches!(
        error.payload,
        AgentEventPayload::Error { ref message } if message == "provider failed"
    ));
    assert!(matches!(finished.payload, AgentEventPayload::Finished));
    assert_eq!(error.routing_generation, finished.routing_generation);
}

#[tokio::test]
async fn old_child_completion_cannot_reach_same_name_reconnected_parent() {
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let mut registry = AgentRegistry::new(event_tx);
    let (_old_parent_peer, old_parent_transport) = loopal_ipc::duplex_pair();
    let (old_parent, _old_parent_incoming) = Connection::new(old_parent_transport).into_listening();
    let (old_completion_tx, _old_completion_rx) = mpsc::channel(1);
    registry
        .register_connection_with_parent("parent", old_parent, None, None, Some(old_completion_tx))
        .unwrap();
    let old_parent_generation = registry.generation("parent").unwrap();

    let (_child_peer, child_transport) = loopal_ipc::duplex_pair();
    let (child, _child_incoming) = Connection::new(child_transport).into_listening();
    registry
        .register_connection_with_parent(
            "child",
            child,
            Some(QualifiedAddress::local("parent")),
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        registry.agents["child"].parent_generation,
        Some(old_parent_generation)
    );

    registry.unregister_connection("parent");
    let (_new_parent_peer, new_parent_transport) = loopal_ipc::duplex_pair();
    let (new_parent, _new_parent_incoming) = Connection::new(new_parent_transport).into_listening();
    let (new_completion_tx, mut new_completion_rx) = mpsc::channel(1);
    registry
        .register_connection_with_parent("parent", new_parent, None, None, Some(new_completion_tx))
        .unwrap();
    assert_ne!(registry.generation("parent"), Some(old_parent_generation));

    let mut pending = registry.emit_agent_completion(
        "child",
        AgentCompletion::goal(Some("old child result".into())),
    );
    assert!(!pending.has_parent_delivery());
    assert!(
        registry
            .local_parent_generation_for_completion("child")
            .is_none()
    );
    pending.deliver_events().await.unwrap();
    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Finished
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), new_completion_rx.recv())
            .await
            .is_err(),
        "same-name replacement parent must not receive an older edge's completion"
    );
}
