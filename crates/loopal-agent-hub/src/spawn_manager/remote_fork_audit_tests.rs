use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_protocol::QualifiedAddress;
use loopal_vault_api::AuditSink;
use tokio::sync::{Mutex, mpsc};

use super::spawn_audit_test_support::Sink;
use super::{PreparedSpawn, SpawnRequestLease};
use crate::types::SpawnAuthority;
use crate::{Hub, HubUplink};

async fn fixture(
    sink: Option<Arc<dyn AuditSink>>,
) -> (
    Arc<Mutex<Hub>>,
    PreparedSpawn,
    Arc<Connection<loopal_ipc::Listening>>,
) {
    let (events, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    let (peer_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (_peer, _peer_rx) = Connection::new(peer_transport).into_listening();
    let (connection, _hub_rx) = Connection::new(hub_transport).into_listening();
    hub.uplink = Some(Arc::new(HubUplink::new(
        connection.clone(),
        "origin".into(),
    )));
    if let Some(sink) = sink {
        hub.set_protected_audit(sink);
    }
    let prepared = PreparedSpawn {
        name: "remote-child".into(),
        request_lease: SpawnRequestLease::TrustedMetaHub(connection.clone()),
        cwd: PathBuf::from("/tmp"),
        prompt: Some("not audited".into()),
        parent: Some(QualifiedAddress::parse("parent@origin")),
        parent_execution: None,
        authority: SpawnAuthority::default(),
        agent_type: None,
        depth: 2,
        fork_context: None,
        workflow_permission_causation: None,
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        notify_parent_on_completion: true,
        root_cwd: PathBuf::from("/tmp"),
        root: "remote-child".into(),
    };
    (Arc::new(Mutex::new(hub)), prepared, connection)
}

#[tokio::test]
async fn destination_spawn_records_trusted_metahub_authority() {
    let sink = Arc::new(Sink::new(false));
    let (hub, prepared, _connection) = fixture(Some(sink.clone())).await;
    super::fork::authorized(&hub, &prepared, || Ok(()))
        .await
        .unwrap();

    let records = sink.records();
    let record = records.first().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(record.spawn_target.as_deref(), Some("remote_destination"));
    assert_eq!(record.subject, "remote-child");
    assert_eq!(record.cwd.as_deref(), Some(PathBuf::from("/tmp").as_path()));
    assert_eq!(record.depth, Some(2));
    assert_eq!(record.agent_name, None);
    assert_eq!(record.session_id, None);
    assert_eq!(record.generation, None);
    assert_eq!(record.workflow_run_id, None);
}

#[tokio::test]
async fn missing_or_failing_audit_prevents_destination_fork() {
    let (hub, prepared, _connection) = fixture(None).await;
    let called = AtomicBool::new(false);
    let error = super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();
    assert_eq!(error, "protected audit unavailable");
    assert!(!called.load(Ordering::SeqCst));

    let sink = Arc::new(Sink::new(true));
    let (hub, prepared, _connection) = fixture(Some(sink.clone())).await;
    let error = super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();
    assert!(error.contains("spawn authority audit failed"));
    assert_eq!(sink.records().len(), 1);
    assert!(!called.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocked_audit_releases_hub_lock_and_revalidates_uplink() {
    let (sink, gate) = Sink::gated();
    let (hub, prepared, connection) = fixture(Some(Arc::new(sink))).await;
    let called = Arc::new(AtomicBool::new(false));
    let task = tokio::spawn({
        let hub = hub.clone();
        let called = called.clone();
        async move {
            super::fork::authorized(&hub, &prepared, move || {
                called.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await
        }
    });

    gate.wait_started().await;
    let mut locked = tokio::time::timeout(Duration::from_millis(200), hub.lock())
        .await
        .expect("audit append must not hold the Hub mutex");
    assert!(locked.is_active_uplink_connection(&connection));
    locked.uplink = None;
    drop(locked);
    gate.release();

    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error, "spawn requester connection lease is stale");
    assert!(!called.load(Ordering::SeqCst));
}
