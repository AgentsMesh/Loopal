use std::sync::atomic::Ordering;
use std::time::Duration;

use super::support::harness;
use crate::workflow::scheduler::WorkflowCleanupStatus;

#[tokio::test(start_paused = true)]
async fn double_timeout_retains_single_flight_custody_until_retry_succeeds() {
    let fixture = harness().await;
    fixture
        .probe
        .block_process_shutdown
        .store(true, Ordering::SeqCst);
    let hub_guard = fixture.spawner.hub.lock().await;
    let stage_timeout = Duration::from_millis(10);

    let first = super::super::control::shutdown_supervisor_for_test(
        &fixture.spawner,
        &fixture.execution,
        stage_timeout,
    );
    tokio::task::yield_now().await;
    tokio::time::advance(stage_timeout).await;
    super::support::wait_for(&fixture.probe.process_shutdowns, 1).await;
    tokio::time::advance(stage_timeout).await;
    tokio::task::yield_now().await;
    tokio::time::advance(stage_timeout).await;
    tokio::task::yield_now().await;
    tokio::time::advance(stage_timeout).await;
    assert_eq!(first.await.unwrap(), WorkflowCleanupStatus::TimedOut);

    let second = tokio::spawn(super::super::control::shutdown(
        &fixture.spawner,
        &fixture.execution,
        Duration::from_secs(1),
    ));
    tokio::task::yield_now().await;
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_execution.contains_key(&fixture.execution));
    assert_eq!(fixture.probe.process_shutdowns.load(Ordering::SeqCst), 1);
    drop(owners);
    drop(hub_guard);

    fixture.probe.process_shutdown_release.notify_one();
    tokio::time::advance(stage_timeout).await;
    assert_eq!(second.await.unwrap(), WorkflowCleanupStatus::Confirmed);
    assert_eq!(fixture.probe.process_shutdowns.load(Ordering::SeqCst), 1);
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_attempt.is_empty());
    assert!(owners.by_execution.is_empty());
}

#[tokio::test]
async fn lost_process_shutdown_custody_retains_tombstone_and_escalates() {
    let fixture = harness().await;
    fixture
        .probe
        .fail_process_shutdown
        .store(true, Ordering::SeqCst);
    let shutdown_signal = fixture.spawner.hub.lock().await.shutdown_signal.clone();
    let shutdown = shutdown_signal.notified();
    tokio::pin!(shutdown);
    shutdown.as_mut().enable();

    assert_eq!(
        super::super::control::shutdown_supervisor_for_test(
            &fixture.spawner,
            &fixture.execution,
            Duration::from_secs(1),
        )
        .await
        .unwrap(),
        WorkflowCleanupStatus::TimedOut
    );
    tokio::time::timeout(Duration::from_secs(1), shutdown)
        .await
        .expect("lost process custody must request Hub shutdown");

    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_execution.contains_key(&fixture.execution));
    let owner = &owners.by_attempt[&fixture.causation.attempt_id];
    assert!(owner.cleanup_registered);
    assert!(owner.process.is_none());
    assert!(owner.process_shutdown.is_some());
}

#[tokio::test(start_paused = true)]
async fn pending_process_shutdown_has_bounded_escalation() {
    let fixture = harness().await;
    fixture
        .probe
        .block_process_shutdown
        .store(true, Ordering::SeqCst);
    let hub_guard = fixture.spawner.hub.lock().await;
    let shutdown_signal = fixture.spawner.shutdown_signal.clone();
    let shutdown = shutdown_signal.notified();
    tokio::pin!(shutdown);
    shutdown.as_mut().enable();

    let timeout = Duration::from_millis(10);
    let cleanup = super::super::control::shutdown_supervisor_for_test(
        &fixture.spawner,
        &fixture.execution,
        timeout,
    );
    tokio::task::yield_now().await;
    for _ in 0..32 {
        tokio::time::advance(timeout).await;
        tokio::task::yield_now().await;
    }

    assert_eq!(cleanup.await.unwrap(), WorkflowCleanupStatus::TimedOut);
    shutdown.await;
    assert_eq!(fixture.probe.process_shutdowns.load(Ordering::SeqCst), 1);
    let owners = fixture.spawner.attempts.lock().await;
    let owner = &owners.by_attempt[&fixture.causation.attempt_id];
    assert!(owner.cleanup_registered);
    assert!(owner.process.is_none());
    assert!(owner.process_shutdown.is_some());
    drop(hub_guard);
}

#[tokio::test(start_paused = true)]
async fn pending_operation_lock_has_bounded_escalation() {
    let fixture = harness().await;
    let operation = fixture.spawner.attempts.lock().await.by_attempt[&fixture.causation.attempt_id]
        .operation
        .clone();
    let operation_guard = operation.lock().await;
    let shutdown = fixture.spawner.shutdown_signal.notified();
    tokio::pin!(shutdown);
    shutdown.as_mut().enable();

    let timeout = Duration::from_millis(10);
    let cleanup = super::super::control::shutdown_supervisor_for_test(
        &fixture.spawner,
        &fixture.execution,
        timeout,
    );
    tokio::task::yield_now().await;
    tokio::time::advance(timeout).await;
    tokio::task::yield_now().await;

    assert_eq!(cleanup.await.unwrap(), WorkflowCleanupStatus::TimedOut);
    shutdown.await;
    assert_eq!(fixture.probe.process_shutdowns.load(Ordering::SeqCst), 0);
    let owners = fixture.spawner.attempts.lock().await;
    let owner = &owners.by_attempt[&fixture.causation.attempt_id];
    assert!(owner.cleanup_registered);
    assert!(owner.process.is_some());
    assert!(owner.process_shutdown.is_none());
    assert!(owners.by_execution.contains_key(&fixture.execution));
    drop(operation_guard);
}

#[tokio::test]
async fn stale_generation_cleanup_does_not_remove_replacement_owner() {
    let fixture = harness().await;
    let stale = fixture.execution.clone();
    let replacement = crate::types::AgentExecutionRef::local(
        stale.address.agent.clone(),
        stale.connection_generation + 1,
    );
    let attempt_id = fixture.causation.attempt_id.clone();
    let mut owners = fixture.spawner.attempts.lock().await;
    owners
        .by_execution
        .insert(replacement.clone(), attempt_id.clone());
    owners.by_attempt.get_mut(&attempt_id).unwrap().execution = replacement.clone();

    assert!(super::super::remove_exact_owner(&mut owners, &stale));
    assert_eq!(owners.by_execution.get(&replacement), Some(&attempt_id));
    assert_eq!(owners.by_attempt[&attempt_id].execution, replacement);
}
