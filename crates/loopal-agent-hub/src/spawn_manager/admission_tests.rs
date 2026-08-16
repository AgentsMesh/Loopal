use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use tokio::sync::{Mutex, mpsc};

use super::SpawnAdmission;
use crate::Hub;
use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::types::{AgentExecutionRef, RegisteredAgent};

async fn admission(
    event_tx: mpsc::Sender<AgentEvent>,
) -> (SpawnAdmission, Arc<Mutex<Hub>>, AgentExecutionRef) {
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (peer, transport) = loopal_ipc::duplex_pair();
    let (_peer, _peer_incoming) = Connection::new(peer).into_listening();
    let (connection, incoming) = Connection::new(transport).into_listening();
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("child", connection.clone(), None, None, None)
        .unwrap();
    let delivery = {
        let locked = hub.lock().await;
        PreparedAuthoritativeEvent::from_hub(&locked, AgentEvent::root(AgentEventPayload::Running))
    };
    let (_completion_tx, completion_rx) = mpsc::channel(1);
    (
        SpawnAdmission {
            hub: hub.clone(),
            name: "child".into(),
            connection: connection.clone(),
            incoming,
            completion_rx,
            delivery,
            parent_name: loopal_protocol::ROOT_AGENT_NAME.into(),
            parent_generation: None,
            cleanup: super::AdmissionCleanup::new(
                hub.clone(),
                connection.clone(),
                execution.clone(),
            ),
            registered: RegisteredAgent {
                agent_id: "id".into(),
                execution: execution.clone(),
            },
        },
        hub,
        execution,
    )
}

#[tokio::test]
async fn closed_event_queue_cleans_up_exact_child() {
    let (event_tx, event_rx) = mpsc::channel(1);
    drop(event_rx);
    let (admission, hub, _) = admission(event_tx).await;

    let error = admission.complete().await.unwrap_err();

    assert!(error.contains("authoritative Hub event queue closed"));
    assert!(
        hub.lock()
            .await
            .registry
            .current_execution("child")
            .is_none()
    );
}

#[tokio::test]
async fn stale_child_before_admission_does_not_remove_reconnect() {
    let (event_tx, mut event_rx) = mpsc::channel(2);
    let (admission, hub, stale) = admission(event_tx).await;
    let connection = admission.connection.clone();
    let current = {
        let mut locked = hub.lock().await;
        assert!(locked.registry.unregister_exact(&stale));
        locked
            .registry
            .register_connection_with_parent_execution("child", connection, None, None, None)
            .unwrap()
    };

    let error = admission.complete().await.unwrap_err();

    assert!(error.contains("reconnected before spawn admission"));
    assert_eq!(
        hub.lock().await.registry.current_execution("child"),
        Some(current)
    );
    assert!(event_rx.recv().await.is_some());
}

#[tokio::test]
async fn stale_parent_after_event_admission_cleans_up_child() {
    let (event_tx, mut event_rx) = mpsc::channel(2);
    let (mut admission, hub, child) = admission(event_tx).await;
    let (parent_connection, _incoming) = {
        let (transport, _peer) = loopal_ipc::duplex_pair();
        Connection::new(transport).into_listening()
    };
    let parent = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("parent", parent_connection, None, None, None)
        .unwrap();
    admission.parent_name = "parent".into();
    admission.parent_generation = Some(parent.connection_generation);
    assert!(hub.lock().await.registry.unregister_exact(&parent));

    let error = admission.complete().await.unwrap_err();

    assert!(error.contains("parent agent 'parent' reconnected"));
    assert!(
        hub.lock()
            .await
            .registry
            .current_execution("child")
            .is_none()
    );
    assert!(hub.lock().await.registry.runtime_facts(&child).is_none());
    assert!(event_rx.recv().await.is_some());
}

#[tokio::test]
async fn stale_topology_registration_removes_only_child_execution() {
    let (event_tx, mut event_rx) = mpsc::channel(2);
    let (admission, hub, execution) = admission(event_tx).await;
    let facts = crate::types::AgentRuntimeFacts::root(
        std::env::temp_dir(),
        crate::types::SpawnAuthority::default(),
    );
    {
        let mut locked = hub.lock().await;
        assert!(locked.registry.set_runtime_facts(&execution, facts.clone()));
        assert!(locked.spawn_registry.register_exact(
            AgentExecutionRef::local("child", execution.connection_generation + 1),
            facts.cwd,
            None,
        ));
    }

    assert_eq!(
        admission.complete().await.unwrap_err(),
        "stale child topology registration"
    );
    assert!(
        hub.lock()
            .await
            .registry
            .current_execution("child")
            .is_none()
    );
    assert!(event_rx.recv().await.is_some());
}

#[tokio::test]
async fn reconnect_during_mcp_cleanup_preserves_new_generation() {
    let (event_tx, _event_rx) = mpsc::channel(2);
    let (admission, hub, stale) = admission(event_tx).await;
    let topology = hub.lock().await.spawn_registry.clone();
    assert!(topology.register_exact(stale.clone(), std::env::temp_dir(), None));
    let current = {
        let mut locked = hub.lock().await;
        assert!(locked.registry.unregister_exact(&stale));
        locked
            .registry
            .register_connection_with_parent_execution(
                "child",
                admission.connection.clone(),
                None,
                None,
                None,
            )
            .unwrap()
    };
    let mcp = hub.lock().await.mcp_service.clone();

    let error = admission
        .ensure_active_after_mcp(&topology, &mcp)
        .await
        .unwrap_err();

    assert_eq!(error, "child reconnected during MCP admission");
    assert_eq!(
        hub.lock().await.registry.current_execution("child"),
        Some(current)
    );
    assert!(topology.cwd_for(&stale).is_none());
}
