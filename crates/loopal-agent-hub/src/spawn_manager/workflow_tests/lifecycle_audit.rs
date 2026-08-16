use std::sync::Arc;

use super::requests::{causation, request};
use super::support::{harness_with_audit, spawner_with_root, wait_for};
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::workflow::scheduler::WorkflowSpawner;

#[tokio::test]
async fn preparation_fails_before_spawn_authority_audit_when_lifecycle_audit_fails() {
    let sink = Arc::new(Sink::new(true));
    let spawner = spawner_with_root(Some(sink.clone())).await;
    let failure = match spawner
        .prepare(request(causation(
            "wrun_prepare",
            "wnode_prepare",
            "watt_prepare",
        )))
        .await
    {
        Err(failure) => failure,
        Ok(_) => panic!("failed lifecycle audit must prevent preparation"),
    };

    assert!(failure.failure.reason.contains("lifecycle audit failed"));
    let records = sink.records();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].op,
        loopal_vault_api::ProtectedOp::WorkflowAttemptLifecycle
    );
    assert_eq!(records[0].workflow_phase.as_deref(), Some("prepare"));
    assert_eq!(spawner.hub.lock().await.registry.agent_count(), 1);
}

#[tokio::test]
async fn activation_waits_for_audit_before_agent_start() {
    let (sink, gate) = Sink::gated();
    let fixture = harness_with_audit(Some(Arc::new(sink))).await;
    let spawner = fixture.spawner.clone();
    let execution = fixture.execution.clone();
    let activation = tokio::spawn(async move { spawner.activate(&execution).await });

    gate.wait_started().await;
    assert_eq!(
        fixture
            .probe
            .starts
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    gate.release();
    assert!(activation.await.unwrap().is_ok());
    wait_for(&fixture.probe.starts, 1).await;
    fixture
        .spawner
        .shutdown_and_wait(&fixture.execution, std::time::Duration::from_secs(1))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_during_activation_audit_prevents_agent_start() {
    let (sink, gate) = Sink::gated();
    let fixture = harness_with_audit(Some(Arc::new(sink))).await;
    let activation = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move { spawner.activate(&execution).await }
    });

    gate.wait_started().await;
    let interruption = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move { spawner.interrupt(&execution).await }
    });
    loop {
        let stopping = fixture
            .spawner
            .attempts
            .lock()
            .await
            .by_attempt
            .get(&fixture.causation.attempt_id)
            .is_some_and(|attempt| attempt.phase == super::super::AttemptPhase::Stopping);
        if stopping {
            break;
        }
        tokio::task::yield_now().await;
    }
    gate.release();

    assert!(matches!(
        activation.await.unwrap(),
        Err(crate::workflow::scheduler::WorkflowActivationFailure::Stopped(_))
    ));
    assert_eq!(
        interruption.await.unwrap(),
        crate::workflow::scheduler::WorkflowStopStatus::Requested
    );
    assert_eq!(
        fixture
            .probe
            .starts
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    fixture
        .spawner
        .shutdown_and_wait(&fixture.execution, std::time::Duration::from_secs(1))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn abort_during_prepare_audit_prevents_child_admission() {
    let (sink, gate) = Sink::gated();
    let spawner = spawner_with_root(Some(Arc::new(sink))).await;
    let causation = causation("wrun_abort_audit", "wnode_abort_audit", "watt_abort_audit");
    let preparation = tokio::spawn({
        let spawner = spawner.clone();
        let request = request(causation.clone());
        async move { spawner.prepare(request).await }
    });

    gate.wait_started().await;
    let abort = tokio::spawn({
        let spawner = spawner.clone();
        let causation = causation.clone();
        async move {
            spawner
                .abort_prepare_and_wait(&causation, std::time::Duration::from_secs(1))
                .await
        }
    });
    loop {
        let cancelled = spawner
            .attempts
            .lock()
            .await
            .preparing
            .get(&causation.attempt_id)
            .is_some_and(|owner| owner.is_cancelled());
        if cancelled {
            break;
        }
        tokio::task::yield_now().await;
    }
    gate.release();

    assert!(preparation.await.unwrap().is_err());
    assert_eq!(
        abort.await.unwrap(),
        crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed
    );
    assert_eq!(spawner.hub.lock().await.registry.agent_count(), 1);
}
