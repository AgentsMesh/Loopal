use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use loopal_ipc::Connection;

use super::support::harness;
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::workflow::scheduler::{
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowSpawner, WorkflowStopStatus,
};

#[tokio::test]
async fn stale_pre_start_lease_cleans_only_its_exact_process_owner() {
    let fixture = harness().await;
    let (_replacement_peer, transport) = loopal_ipc::duplex_pair();
    let replacement = {
        let mut hub = fixture.spawner.hub.lock().await;
        assert!(hub.registry.unregister_exact(&fixture.execution));
        let connection = Connection::new(transport).into_listening().0;
        hub.registry
            .register_connection_with_parent_execution(
                &fixture.execution.address.agent,
                connection,
                None,
                None,
                None,
            )
            .unwrap()
    };

    assert!(matches!(
        fixture.spawner.activate(&fixture.execution).await,
        Err(WorkflowActivationFailure::Stopped(_))
    ));
    assert_eq!(fixture.probe.starts.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.probe.shutdowns.load(Ordering::SeqCst), 1);
    assert!(fixture.probe.process_stopped.load(Ordering::SeqCst));
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_attempt.is_empty());
    assert!(owners.by_execution.is_empty());
    drop(owners);
    assert_eq!(
        fixture
            .spawner
            .hub
            .lock()
            .await
            .registry
            .current_execution(&fixture.execution.address.agent),
        Some(replacement)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_during_activation_audit_prevents_agent_start() {
    let (sink, gate) = Sink::gated();
    let fixture = super::support::harness_with_audit(Some(Arc::new(sink))).await;
    let activation = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move { spawner.activate(&execution).await }
    });
    gate.wait_started().await;
    let shutdown = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move {
            spawner
                .shutdown_and_wait(&execution, Duration::from_secs(1))
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let stopping = fixture.spawner.attempts.lock().await.by_attempt
                [&fixture.causation.attempt_id]
                .phase
                == super::super::AttemptPhase::Stopping;
            if stopping {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    gate.release();

    assert!(matches!(
        activation.await.unwrap(),
        Err(WorkflowActivationFailure::Stopped(_))
    ));
    assert_eq!(shutdown.await.unwrap(), WorkflowCleanupStatus::Confirmed);
    assert_eq!(fixture.probe.starts.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.probe.shutdowns.load(Ordering::SeqCst), 1);
    assert!(fixture.probe.process_stopped.load(Ordering::SeqCst));
    assert!(fixture.spawner.attempts.lock().await.by_attempt.is_empty());
}

#[tokio::test]
async fn interrupt_before_activation_still_cleans_the_prepared_owner() {
    let fixture = harness().await;
    assert_eq!(
        fixture.spawner.interrupt(&fixture.execution).await,
        WorkflowStopStatus::Requested
    );
    assert!(matches!(
        fixture.spawner.activate(&fixture.execution).await,
        Err(WorkflowActivationFailure::Stopped(_))
    ));

    assert_eq!(fixture.probe.starts.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.probe.shutdowns.load(Ordering::SeqCst), 1);
    assert!(fixture.probe.process_stopped.load(Ordering::SeqCst));
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_attempt.is_empty());
    assert!(owners.by_execution.is_empty());
}

#[tokio::test]
async fn timed_out_shutdown_keeps_background_exact_cleanup_alive() {
    let fixture = harness().await;
    fixture
        .probe
        .reply_to_shutdown
        .store(false, Ordering::SeqCst);

    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&fixture.execution, Duration::from_millis(20))
            .await,
        WorkflowCleanupStatus::TimedOut
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let owners = fixture.spawner.attempts.lock().await;
            if fixture.probe.process_stopped.load(Ordering::SeqCst)
                && owners.by_attempt.is_empty()
                && owners.by_execution.is_empty()
            {
                return;
            }
            drop(owners);
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cancelled_wait_while_operation_is_locked_keeps_cleanup_alive() {
    let fixture = harness().await;
    let operation = {
        fixture.spawner.attempts.lock().await.by_attempt[&fixture.causation.attempt_id]
            .operation
            .clone()
    };
    let operation_guard = operation.lock().await;
    let waiter = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move {
            spawner
                .shutdown_and_wait(&execution, Duration::from_secs(1))
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while Arc::strong_count(&operation) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    drop(operation_guard);

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let owners = fixture.spawner.attempts.lock().await;
            if fixture.probe.process_stopped.load(Ordering::SeqCst) && owners.by_attempt.is_empty()
            {
                return;
            }
            drop(owners);
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
