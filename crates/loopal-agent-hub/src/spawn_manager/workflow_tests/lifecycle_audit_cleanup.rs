use std::sync::Arc;

use super::support::{harness_with_audit, wait_for};
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::workflow::scheduler::WorkflowSpawner;

#[tokio::test]
async fn failed_activation_audit_blocks_start_but_cleanup_still_completes() {
    let sink = Arc::new(Sink::new(true));
    let fixture = harness_with_audit(Some(sink.clone())).await;

    assert!(matches!(
        fixture.spawner.activate(&fixture.execution).await,
        Err(crate::workflow::scheduler::WorkflowActivationFailure::Stopped(_))
    ));
    assert_eq!(
        fixture
            .probe
            .starts
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(
        fixture
            .probe
            .shutdowns
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert!(
        fixture
            .probe
            .process_stopped
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    assert!(fixture.spawner.attempts.lock().await.by_attempt.is_empty());
    let phases: Vec<_> = sink
        .records()
        .into_iter()
        .filter_map(|record| record.workflow_phase)
        .collect();
    assert_eq!(phases, ["activate", "shutdown"]);
}

#[tokio::test]
async fn interrupt_and_shutdown_continue_when_cleanup_audit_fails() {
    let sink = Arc::new(Sink::new(true));
    let fixture = harness_with_audit(Some(sink.clone())).await;

    assert_eq!(
        fixture.spawner.interrupt(&fixture.execution).await,
        crate::workflow::scheduler::WorkflowStopStatus::Requested
    );
    wait_for(&fixture.probe.interrupts, 1).await;
    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&fixture.execution, std::time::Duration::from_secs(1))
            .await,
        crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed
    );
    assert_eq!(
        fixture
            .probe
            .shutdowns
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert!(
        fixture
            .probe
            .process_stopped
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    let phases: Vec<_> = sink
        .records()
        .into_iter()
        .filter_map(|record| record.workflow_phase)
        .collect();
    assert_eq!(phases, ["interrupt", "shutdown"]);
}
