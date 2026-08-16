use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use loopal_vault_api::{AuditMetadata, AuditResult, AuditSink, ProtectedOp, VaultOp};

use super::SpawnRequestLease;
use super::spawn_audit_test_support::{Sink, agent_fixture};

struct PanicSink;

impl AuditSink for PanicSink {
    fn record(&self, _: VaultOp, _: &str, _: &AuditMetadata<'_>) -> AuditResult<()> {
        Ok(())
    }

    fn record_protected(&self, _: ProtectedOp, _: &str, _: &AuditMetadata<'_>) -> AuditResult<()> {
        panic!("audit worker panic")
    }
}

#[tokio::test]
async fn local_spawn_records_authenticated_authority_before_fork() {
    let sink = Arc::new(Sink::new(false));
    let (hub, prepared, execution) = agent_fixture(Some(sink.clone())).await;
    let called = AtomicBool::new(false);

    super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap();

    assert!(called.load(Ordering::SeqCst));
    let records = sink.records();
    let record = records.first().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(record.op, ProtectedOp::SpawnAuthority);
    assert_eq!(record.subject, "child");
    assert_eq!(record.session_id.as_deref(), Some("session-spawn"));
    assert_eq!(record.cwd.as_deref(), Some(PathBuf::from("/tmp").as_path()));
    assert_eq!(record.agent_name.as_deref(), Some("parent"));
    assert_eq!(record.depth, Some(1));
    assert_eq!(record.generation, Some(execution.connection_generation));
    assert_eq!(record.workflow_run_id.as_deref(), Some("wrun_spawn"));
    assert_eq!(record.workflow_node_id.as_deref(), Some("wnode_spawn"));
    assert_eq!(record.workflow_attempt_id.as_deref(), Some("watt_spawn"));
    assert_eq!(record.spawn_target.as_deref(), Some("local"));
    assert_eq!(
        record.model.as_deref(),
        Some(prepared.authority.model.as_str())
    );
    assert_eq!(
        record.permission_mode.as_deref(),
        Some(prepared.authority.permission_mode.to_string().as_str())
    );
    assert_eq!(
        record.decision_mode.as_deref(),
        Some(prepared.authority.decision_mode.to_string().as_str())
    );
    assert_eq!(
        record.sandbox_policy.as_deref(),
        Some(prepared.authority.sandbox_policy.to_string().as_str())
    );
}

#[tokio::test]
async fn missing_or_failing_audit_prevents_fork() {
    let (hub, prepared, _) = agent_fixture(None).await;
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
    let (hub, prepared, _) = agent_fixture(Some(sink.clone())).await;
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

#[tokio::test]
async fn unauditable_internal_authority_and_worker_panic_prevent_fork() {
    let (hub, mut prepared, _) = agent_fixture(Some(Arc::new(Sink::new(false)))).await;
    prepared.request_lease = SpawnRequestLease::Internal;
    prepared.parent_execution = None;
    let called = AtomicBool::new(false);
    let error = super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();
    assert_eq!(error, "internal process spawn authority is unauditable");
    assert!(!called.load(Ordering::SeqCst));

    let (hub, prepared, _) = agent_fixture(Some(Arc::new(PanicSink))).await;
    let error = super::fork::authorized(&hub, &prepared, || {
        called.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();
    assert!(error.contains("spawn authority audit task failed"));
    assert!(!called.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocked_audit_releases_hub_lock_and_revalidates_agent_lease() {
    let (sink, gate) = Sink::gated();
    let (hub, prepared, execution) = agent_fixture(Some(Arc::new(sink))).await;
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
    assert!(locked.registry.unregister_exact(&execution));
    drop(locked);
    gate.release();

    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error, "spawn requester connection lease is stale");
    assert!(!called.load(Ordering::SeqCst));
}
