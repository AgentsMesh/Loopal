use std::sync::Arc;
use std::time::Duration;

use super::super::PreparationOwner;
use super::requests::{causation, request};
use super::support::{harness, spawner_with_root};
use crate::workflow::scheduler::{WorkflowCleanupStatus, WorkflowSpawner};

#[tokio::test]
async fn abort_requires_the_full_causation_owner() {
    let fixture = harness().await;
    let mismatched = causation(
        "wrun_other",
        "wnode_other",
        fixture.causation.attempt_id.as_str(),
    );

    assert_eq!(
        fixture
            .spawner
            .abort_prepare_and_wait(&mismatched, Duration::from_millis(20))
            .await,
        WorkflowCleanupStatus::TimedOut
    );
    assert!(
        fixture
            .spawner
            .attempts
            .lock()
            .await
            .by_execution
            .contains_key(&fixture.execution)
    );
    assert_eq!(
        fixture
            .spawner
            .abort_prepare_and_wait(&fixture.causation, Duration::from_secs(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
    let failure = match fixture.spawner.prepare(request(mismatched)).await {
        Ok(_) => panic!("pre-aborted causation unexpectedly prepared a worker"),
        Err(failure) => failure,
    };
    assert!(failure.failure.reason.contains("cancelled"));
    assert!(fixture.spawner.attempts.lock().await.pre_aborted.is_empty());
}

#[tokio::test]
async fn mismatched_preparation_does_not_suppress_an_exact_pre_abort() {
    let spawner = spawner_with_root(Some(Arc::new(loopal_vault_api::NoopAuditSink))).await;
    let target = causation("wrun_target", "wnode_target", "watt_collision");
    let mismatched = causation("wrun_other", "wnode_other", "watt_collision");
    let preparation = Arc::new(PreparationOwner::new(mismatched));
    spawner
        .attempts
        .lock()
        .await
        .preparing
        .insert(target.attempt_id.clone(), preparation.clone());
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
    assert!(!preparation.is_cancelled());
    assert!(!abort.is_finished());
    spawner
        .attempts
        .lock()
        .await
        .preparing
        .remove(&target.attempt_id);
    spawner.changed.notify_waiters();
    assert!(spawner.prepare(request(target.clone())).await.is_err());
    assert_eq!(abort.await.unwrap(), WorkflowCleanupStatus::Confirmed);
}

async fn wait_for_tombstone(
    spawner: &super::super::ProductionWorkflowSpawner,
    causation: &loopal_protocol::WorkflowPermissionCausation,
) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let found = spawner
                .attempts
                .lock()
                .await
                .pre_aborted
                .get(&causation.attempt_id)
                .is_some_and(|values| values.iter().any(|value| value == causation));
            if found {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
