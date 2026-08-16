use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_protocol::{AgentEvent, QualifiedAddress};
use tokio::sync::{Mutex, mpsc};

use super::{Registration, await_admission, install_runtime_authority, register};
use crate::Hub;
use crate::spawn_manager::SpawnRequestLease;
use crate::types::{AgentExecutionRef, AgentRuntimeFacts, SpawnAuthority};

fn connection() -> (
    Arc<Connection<loopal_ipc::Listening>>,
    mpsc::Receiver<Incoming>,
) {
    let (transport, _peer) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening()
}

fn facts(parent: Option<AgentExecutionRef>) -> AgentRuntimeFacts {
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.parent = parent;
    facts
}

fn registration(request_lease: SpawnRequestLease) -> Registration {
    Registration {
        name: "child".into(),
        request_lease,
        parent: None,
        expected_parent: None,
        model: None,
        session_id: None,
        notify_parent_on_completion: true,
        mark_running: true,
        facts: facts(None),
    }
}

#[tokio::test]
async fn stale_request_lease_rejects_registration_without_side_effects() {
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(2);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let stale = AgentExecutionRef::local("requester", 1);
    let (connection, incoming) = connection();

    let error = register(
        hub.clone(),
        connection,
        incoming,
        registration(SpawnRequestLease::Agent(stale)),
    )
    .await
    .unwrap_err();

    assert_eq!(error, "spawn requester connection lease is stale");
    assert!(
        hub.lock()
            .await
            .registry
            .current_execution("child")
            .is_none()
    );
}

#[tokio::test]
async fn stale_runtime_authority_install_unregisters_only_exact_execution() {
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(2);
    let mut hub = Hub::new(event_tx);
    let (stale_connection, _incoming) = connection();
    let stale = hub
        .registry
        .register_connection_with_parent_execution("child", stale_connection, None, None, None)
        .unwrap();
    assert!(hub.registry.unregister_exact(&stale));
    let (current_connection, _incoming) = connection();
    let current = hub
        .registry
        .register_connection_with_parent_execution("child", current_connection, None, None, None)
        .unwrap();

    let error = install_runtime_authority(&mut hub, &stale, facts(None)).unwrap_err();

    assert_eq!(error, "agent runtime authority registration failed");
    assert_eq!(hub.registry.current_execution("child"), Some(current));
}

#[tokio::test]
async fn panicked_admission_invalidates_hub_and_removes_exact_child() {
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(2);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let shutdown = hub.lock().await.shutdown_signal.clone();
    let (connection, _incoming) = connection();
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("child", connection.clone(), None, None, None)
        .unwrap();
    let coordinator = tokio::spawn(async {
        panic!("coordinator panic");
        #[allow(unreachable_code)]
        Ok(crate::types::RegisteredAgent {
            agent_id: String::new(),
            execution: AgentExecutionRef::local("unused", 0),
        })
    });

    let error = await_admission(hub.clone(), connection.clone(), execution, coordinator)
        .await
        .unwrap_err();

    assert!(error.contains("admission coordinator failed"));
    assert!(
        hub.lock()
            .await
            .registry
            .current_execution("child")
            .is_none()
    );
    assert!(!connection.is_connected());
    tokio::time::timeout(Duration::from_millis(100), shutdown.notified())
        .await
        .unwrap();
}

#[tokio::test]
async fn panicked_stale_admission_preserves_reconnected_child() {
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(2);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (stale_connection, _incoming) = connection();
    let stale = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution(
            "child",
            stale_connection.clone(),
            None,
            None,
            None,
        )
        .unwrap();
    let current = {
        let mut locked = hub.lock().await;
        assert!(locked.registry.unregister_exact(&stale));
        let (current_connection, _incoming) = connection();
        locked
            .registry
            .register_connection_with_parent_execution(
                "child",
                current_connection,
                None,
                None,
                None,
            )
            .unwrap()
    };
    let coordinator = tokio::spawn(async {
        panic!("coordinator panic");
        #[allow(unreachable_code)]
        Ok(crate::types::RegisteredAgent {
            agent_id: String::new(),
            execution: AgentExecutionRef::local("unused", 0),
        })
    });

    await_admission(hub.clone(), stale_connection, stale, coordinator)
        .await
        .unwrap_err();

    assert_eq!(
        hub.lock().await.registry.current_execution("child"),
        Some(current)
    );
}

#[test]
fn remote_parent_event_has_no_local_generation() {
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(2);
    let hub = Hub::new(event_tx);
    let mut registration = registration(SpawnRequestLease::Internal);
    registration.parent = Some(QualifiedAddress::parse("remote/parent"));

    let (_, parent, generation) = super::prepare_event(&hub, &registration, "id".into());

    assert_eq!(parent, "parent");
    assert_eq!(generation, None);
}
