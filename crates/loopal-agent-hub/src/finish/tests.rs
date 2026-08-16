use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};
use tokio::sync::mpsc;

use super::*;
use crate::pending_relay::PendingPlanApprovalInfo;

#[tokio::test]
async fn completion_events_backpressure_in_order_without_holding_the_hub_lock() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    hub.lock()
        .await
        .registry
        .register_connection("worker", connection.clone())
        .unwrap();

    let finishing = tokio::spawn({
        let hub = hub.clone();
        let connection = connection.clone();
        async move {
            finish_and_deliver(
                &hub,
                "worker",
                AgentCompletion::new("error", Some("provider failed".into())),
                &connection,
            )
            .await;
        }
    });
    tokio::task::yield_now().await;
    assert!(!finishing.is_finished());
    let guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
        .await
        .expect("completion backpressure must not hold the Hub lock");
    drop(guard);

    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Running
    ));
    let error = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("synthetic Error enqueue timed out")
        .unwrap();
    assert!(matches!(
        error.payload,
        AgentEventPayload::Error { ref message } if message == "provider failed"
    ));
    let finished = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("Finished enqueue timed out")
        .unwrap();
    assert!(matches!(finished.payload, AgentEventPayload::Finished));
    assert_eq!(error.routing_generation, finished.routing_generation);
    assert!(error.routing_generation.is_some());
    tokio::time::timeout(Duration::from_millis(100), finishing)
        .await
        .expect("completion delivery did not finish")
        .unwrap();
}

#[tokio::test]
async fn completion_routes_to_remote_parent_through_uplink() {
    let (event_tx, mut event_rx) = mpsc::channel(8);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (child_peer, child_transport) = loopal_ipc::duplex_pair();
    let (_child, _child_rx) = Connection::new(child_peer).into_listening();
    let (child, _incoming) = Connection::new(child_transport).into_listening();
    let (uplink_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (uplink_connection, _uplink_rx) = Connection::new(uplink_transport).into_listening();
    let (meta, mut meta_rx) = Connection::new(meta_transport).into_listening();
    let execution = {
        let mut h = hub.lock().await;
        h.uplink = Some(Arc::new(crate::HubUplink::new(
            uplink_connection,
            "origin".into(),
        )));
        h.registry
            .register_connection_with_parent_execution(
                "worker",
                child.clone(),
                Some(QualifiedAddress::parse("remote/parent")),
                None,
                None,
            )
            .unwrap()
    };
    let peer = tokio::spawn(async move {
        let Incoming::Request { id, method, params } = meta_rx.recv().await.unwrap() else {
            panic!("expected routed completion")
        };
        assert_eq!(method, loopal_ipc::protocol::methods::META_ROUTE.name);
        let envelope: Envelope = serde_json::from_value(params).unwrap();
        assert_eq!(envelope.target, QualifiedAddress::parse("remote/parent"));
        assert_eq!(envelope.agent_completion.unwrap().reason, "goal");
        meta.respond(id, serde_json::json!({})).await.unwrap();
    });

    finish_and_deliver_exact(
        &hub,
        "worker",
        AgentCompletion::goal(Some("done".into())),
        &child,
        &execution,
    )
    .await;
    peer.await.unwrap();
}

#[tokio::test]
async fn stale_finish_does_not_detach_or_clean_reconnected_agent() {
    let (_old_peer_transport, old_hub_transport) = loopal_ipc::duplex_pair();
    let (old_hub, _old_rx) = Connection::new(old_hub_transport).into_listening();
    let (_new_peer_transport, new_hub_transport) = loopal_ipc::duplex_pair();
    let (new_hub, _new_rx) = Connection::new(new_hub_transport).into_listening();
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let grant_seed = loopal_protocol::PermissionIntentRequest::create(
        "grant",
        "Bash",
        serde_json::json!({}),
        serde_json::json!({}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap()
    .intent_seed;
    {
        let mut h = hub.lock().await;
        h.registry
            .register_connection("worker", old_hub.clone())
            .unwrap();
        h.registry.unregister_connection("worker");
        h.registry
            .register_connection("worker", new_hub.clone())
            .unwrap();
        let new_execution = h.registry.current_execution("worker").unwrap();
        h.pending_plan_approvals.insert(
            ("worker".into(), "reused".into()),
            PendingPlanApprovalInfo {
                agent_conn: new_hub.clone(),
                agent_ipc_id: 1,
                agent_name: "worker".into(),
                interaction_id: "new-generation-token".into(),
                logical_id: "reused".into(),
            },
        );
        h.grant_permission(new_execution, &grant_seed);
    }

    finish_and_deliver(
        &hub,
        "worker",
        AgentCompletion::goal(Some("stale output".into())),
        &old_hub,
    )
    .await;

    let h = hub.lock().await;
    assert!(Arc::ptr_eq(
        &h.registry.get_agent_connection("worker").unwrap(),
        &new_hub
    ));
    assert!(
        h.pending_plan_approvals
            .contains_key(&("worker".into(), "reused".into()))
    );
    let current = h.registry.current_execution("worker").unwrap();
    assert!(h.has_permission_grant(&current, &grant_seed));
    assert_eq!(h.registry.completion_output("worker"), None);
}
