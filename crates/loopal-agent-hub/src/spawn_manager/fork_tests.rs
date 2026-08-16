use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_protocol::QualifiedAddress;
use tokio::sync::{Mutex, mpsc};

use super::{PreparedSpawn, SpawnRequestLease};
use crate::Hub;
use crate::types::{AgentExecutionRef, SpawnAuthority};

fn prepared(parent: AgentExecutionRef) -> PreparedSpawn {
    PreparedSpawn {
        name: "child".into(),
        request_lease: SpawnRequestLease::Agent(parent.clone()),
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

async fn hub_with_parent() -> (Arc<Mutex<Hub>>, AgentExecutionRef) {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (transport, _peer) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    let mut locked = hub.lock().await;
    locked
        .registry
        .register_connection("parent", connection.clone())
        .unwrap();
    let execution = locked
        .registry
        .execution_for_connection("parent", &connection)
        .unwrap();
    let facts =
        crate::types::AgentRuntimeFacts::root(PathBuf::from("/tmp"), SpawnAuthority::default());
    assert!(locked.registry.set_runtime_facts(&execution, facts));
    locked.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    drop(locked);
    (hub, execution)
}

#[tokio::test]
async fn stale_requester_is_rejected_before_fork_side_effect() {
    let (hub, execution) = hub_with_parent().await;
    hub.lock().await.registry.unregister_connection("parent");
    let called = AtomicBool::new(false);
    let error = super::fork::authorized(&hub, &prepared(execution), || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();
    assert!(error.contains("requester connection lease is stale"));
    assert!(!called.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn requester_generation_cannot_change_between_check_and_fork() {
    let (hub, execution) = hub_with_parent().await;
    let competing_hub = hub.clone();
    let (acquired_tx, acquired_rx) = std::sync::mpsc::channel();

    super::fork::authorized(&hub, &prepared(execution), || {
        std::thread::spawn(move || {
            let _locked = competing_hub.blocking_lock();
            acquired_tx.send(()).unwrap();
        });
        assert!(acquired_rx.recv_timeout(Duration::from_millis(20)).is_err());
        Ok(())
    })
    .await
    .unwrap();

    acquired_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("Hub lock should become available immediately after fork returns");
}
