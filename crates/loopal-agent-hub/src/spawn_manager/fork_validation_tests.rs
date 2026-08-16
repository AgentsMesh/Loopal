use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use loopal_ipc::Connection;
use loopal_protocol::QualifiedAddress;
use tokio::sync::{Mutex, mpsc};

use super::{PreparedSpawn, SpawnRequestLease};
use crate::Hub;
use crate::types::{AgentExecutionRef, SpawnAuthority};

async fn register(hub: &Arc<Mutex<Hub>>, name: &str) -> AgentExecutionRef {
    let (transport, _peer) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    let mut locked = hub.lock().await;
    locked
        .registry
        .register_connection(name, connection.clone())
        .unwrap();
    locked
        .registry
        .execution_for_connection(name, &connection)
        .unwrap()
}

fn prepared(requester: AgentExecutionRef, parent: AgentExecutionRef) -> PreparedSpawn {
    PreparedSpawn {
        name: "child".into(),
        request_lease: SpawnRequestLease::Agent(requester),
        cwd: PathBuf::from("/tmp"),
        prompt: None,
        parent: Some(QualifiedAddress::local("parent")),
        parent_execution: Some(parent),
        authority: SpawnAuthority::default(),
        agent_type: None,
        depth: 1,
        fork_context: None,
        workflow_permission_causation: None,
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        notify_parent_on_completion: true,
        root_cwd: PathBuf::from("/tmp"),
        root: "root".into(),
    }
}

async fn harness() -> (Arc<Mutex<Hub>>, PreparedSpawn) {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let requester = register(&hub, "requester").await;
    let parent = register(&hub, "parent").await;
    let prepared = prepared(requester, parent);
    (hub, prepared)
}

#[tokio::test]
async fn stale_parent_is_rejected_before_fork_side_effect() {
    let (hub, prepared) = harness().await;
    hub.lock().await.registry.unregister_connection("parent");
    let called = AtomicBool::new(false);

    let error = super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();

    assert!(error.contains("spawn parent connection lease is stale"));
    assert!(!called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn exhausted_budget_is_rejected_before_fork_side_effect() {
    let (hub, prepared) = harness().await;
    hub.lock().await.max_total_agents = 0;
    let called = AtomicBool::new(false);

    let error = super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();

    assert!(error.contains("Spawn budget exhausted (0/0 sub-agents)"));
    assert!(!called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn duplicate_name_is_rejected_before_fork_side_effect() {
    let (hub, prepared) = harness().await;
    register(&hub, "child").await;
    let called = AtomicBool::new(false);

    let error = super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();

    assert!(error.contains("agent 'child' already registered"));
    assert!(!called.load(Ordering::SeqCst));
}
