use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use tokio::sync::{Mutex, mpsc};

use super::handle_interrupt;
use crate::Hub;
use crate::pending_relay::PendingPlanApprovalInfo;

#[tokio::test]
async fn delayed_old_interrupt_ack_does_not_cancel_reconnected_agent() {
    let (old_agent_transport, old_hub_transport) = loopal_ipc::duplex_pair();
    let (old_agent, mut old_agent_rx) = Connection::new(old_agent_transport).into_listening();
    let (old_hub, _old_hub_rx) = Connection::new(old_hub_transport).into_listening();
    let (new_agent_transport, new_hub_transport) = loopal_ipc::duplex_pair();
    let (_new_agent, _new_agent_rx) = Connection::new(new_agent_transport).into_listening();
    let (new_hub, _new_hub_rx) = Connection::new(new_hub_transport).into_listening();
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    hub.lock()
        .await
        .registry
        .register_connection("main", old_hub.clone())
        .unwrap();

    let interrupt = tokio::spawn({
        let hub = hub.clone();
        async move { handle_interrupt(&hub, serde_json::json!({"target": "main"})).await }
    });
    let interrupt_id = match old_agent_rx.recv().await.unwrap() {
        Incoming::Request { id, method, .. } => {
            assert_eq!(method, "agent/interrupt");
            id
        }
        other => panic!("expected interrupt request, got {other:?}"),
    };

    {
        let mut h = hub.lock().await;
        h.registry.unregister_connection("main");
        h.registry
            .register_connection("main", new_hub.clone())
            .unwrap();
        h.pending_plan_approvals.insert(
            ("main".into(), "reused".into()),
            PendingPlanApprovalInfo {
                agent_conn: new_hub.clone(),
                agent_ipc_id: 1,
                agent_name: "main".into(),
                interaction_id: "new-generation-token".into(),
                logical_id: "reused".into(),
            },
        );
    }
    old_agent
        .respond(interrupt_id, serde_json::json!({"ok": true}))
        .await
        .unwrap();
    interrupt.await.unwrap().unwrap();

    let h = hub.lock().await;
    assert!(
        h.pending_plan_approvals
            .contains_key(&("main".into(), "reused".into()))
    );
    assert!(Arc::ptr_eq(
        &h.registry.get_agent_connection("main").unwrap(),
        &new_hub
    ));
}
