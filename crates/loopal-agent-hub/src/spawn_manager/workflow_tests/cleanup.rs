use std::sync::Arc;
use std::time::Duration;

use super::super::PreparationOwner;
use super::requests::{causation, request};
use super::support::{harness, spawner_with_root};
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::workflow::scheduler::{WorkflowCleanupStatus, WorkflowSpawner};

#[tokio::test]
async fn abort_waits_for_in_flight_preparation_cleanup() {
    let fixture = harness().await;
    let pending = causation("wrun_pending", "wnode_pending", "watt_pending");
    fixture.spawner.attempts.lock().await.preparing.insert(
        pending.attempt_id.clone(),
        Arc::new(PreparationOwner::new(pending.clone())),
    );
    let spawner = fixture.spawner.clone();
    let abort_causation = pending.clone();
    let abort = tokio::spawn(async move {
        spawner
            .abort_prepare_and_wait(&abort_causation, Duration::from_secs(1))
            .await
    });
    loop {
        if fixture
            .spawner
            .attempts
            .lock()
            .await
            .preparing
            .get(&pending.attempt_id)
            .is_some_and(|owner| owner.is_cancelled())
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    fixture
        .spawner
        .attempts
        .lock()
        .await
        .preparing
        .remove(&pending.attempt_id);
    fixture.spawner.changed.notify_waiters();
    assert_eq!(abort.await.unwrap(), WorkflowCleanupStatus::Confirmed);
}

#[tokio::test]
async fn abort_cleans_orphaned_workflow_registration() {
    let spawner = spawner_with_root(None).await;
    let causation = causation("wrun_orphan", "wnode_orphan", "watt_orphan");
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = loopal_ipc::Connection::new(transport).into_listening().0;
    {
        let mut hub = spawner.hub.lock().await;
        let execution = hub
            .registry
            .register_connection_with_parent_execution("orphan", connection, None, None, None)
            .unwrap();
        let mut facts = crate::types::AgentRuntimeFacts::root(
            std::env::temp_dir(),
            crate::types::SpawnAuthority::default(),
        );
        facts.workflow_permission_causation = Some(causation.clone());
        assert!(hub.registry.set_runtime_facts(&execution, facts));
    }

    assert_eq!(
        spawner
            .abort_prepare_and_wait(&causation, Duration::from_secs(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
    assert!(
        spawner
            .hub
            .lock()
            .await
            .registry
            .current_execution("orphan")
            .is_none()
    );
    assert_eq!(tombstone_count(&spawner, &causation).await, 0);
}

#[tokio::test]
async fn pre_abort_waits_for_matching_prepare_to_acknowledge_cancellation() {
    let sink = Arc::new(Sink::new(false));
    let spawner = spawner_with_root(Some(sink.clone())).await;
    let causation = causation("wrun_pre_abort", "wnode_pre_abort", "watt_pre_abort");
    let abort = tokio::spawn({
        let spawner = spawner.clone();
        let causation = causation.clone();
        async move {
            spawner
                .abort_prepare_and_wait(&causation, Duration::from_secs(1))
                .await
        }
    });
    wait_for_tombstone(&spawner, &causation).await;
    assert!(!abort.is_finished());

    assert_eq!(
        spawner
            .abort_prepare_and_wait(&causation, Duration::from_millis(10))
            .await,
        WorkflowCleanupStatus::TimedOut
    );
    assert_eq!(tombstone_count(&spawner, &causation).await, 1);
    let failure = match spawner.prepare(request(causation.clone())).await {
        Err(failure) => failure,
        Ok(_) => panic!("pre-aborted preparation must be rejected"),
    };
    assert!(failure.failure.reason.contains("cancelled"));
    assert_eq!(abort.await.unwrap(), WorkflowCleanupStatus::Confirmed);

    let owners = spawner.attempts.lock().await;
    assert!(owners.pre_aborted.is_empty());
    assert!(owners.preparing.is_empty());
    assert!(owners.by_attempt.is_empty());
    drop(owners);
    assert_eq!(spawner.hub.lock().await.registry.agent_count(), 1);
    assert!(sink.records().is_empty());
}

#[tokio::test]
async fn pre_abort_preserves_mismatched_full_causation_until_exact_ack() {
    let spawner = spawner_with_root(Some(Arc::new(loopal_vault_api::NoopAuditSink))).await;
    let target = causation("wrun_target", "wnode_target", "watt_shared");
    let mismatched = causation("wrun_other", "wnode_other", "watt_shared");
    let abort = tokio::spawn({
        let spawner = spawner.clone();
        let target = target.clone();
        async move {
            spawner
                .abort_prepare_and_wait(&target, Duration::from_secs(1))
                .await
        }
    });
    wait_for_tombstone(&spawner, &target).await;

    let mut mismatched_request = request(mismatched);
    mismatched_request.owner.root_agent.agent = "missing-root".into();
    let failure = match spawner.prepare(mismatched_request).await {
        Err(failure) => failure,
        Ok(_) => panic!("invalid mismatched request must fail"),
    };
    assert!(!failure.failure.reason.contains("cancelled"));
    assert_eq!(tombstone_count(&spawner, &target).await, 1);
    assert!(!abort.is_finished());

    assert!(spawner.prepare(request(target.clone())).await.is_err());
    assert_eq!(abort.await.unwrap(), WorkflowCleanupStatus::Confirmed);
    assert!(spawner.attempts.lock().await.pre_aborted.is_empty());
    assert_eq!(spawner.hub.lock().await.registry.agent_count(), 1);
}

async fn wait_for_tombstone(
    spawner: &super::super::ProductionWorkflowSpawner,
    causation: &loopal_protocol::WorkflowPermissionCausation,
) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if tombstone_count(spawner, causation).await == 1 {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

async fn tombstone_count(
    spawner: &super::super::ProductionWorkflowSpawner,
    causation: &loopal_protocol::WorkflowPermissionCausation,
) -> usize {
    spawner
        .attempts
        .lock()
        .await
        .pre_aborted
        .get(&causation.attempt_id)
        .map_or(0, |values| {
            values
                .iter()
                .filter(|current| *current == causation)
                .count()
        })
}
