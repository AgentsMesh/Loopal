use std::time::Duration;

use loopal_protocol::WorkflowPermissionCausation;

use super::super::ProductionWorkflowSpawner;
use crate::workflow::scheduler::WorkflowCleanupStatus;

pub(in crate::spawn_manager::workflow) async fn abort_prepare(
    spawner: &ProductionWorkflowSpawner,
    causation: &WorkflowPermissionCausation,
    timeout: Duration,
) -> WorkflowCleanupStatus {
    {
        let mut owners = spawner.attempts.lock().await;
        let execution = owners
            .by_attempt
            .get(&causation.attempt_id)
            .filter(|owner| owner.causation == *causation)
            .map(|owner| owner.execution.clone());
        if let Some(execution) = execution {
            drop(owners);
            return super::cleanup::shutdown(spawner, &execution, timeout).await;
        }
        let preparation = owners
            .preparing
            .get(&causation.attempt_id)
            .filter(|owner| owner.causation == *causation)
            .cloned();
        if let Some(preparation) = preparation {
            preparation.cancel();
            drop(owners);
            return wait_for_removal(spawner, causation, timeout).await;
        }
        let tombstones = owners
            .pre_aborted
            .entry(causation.attempt_id.clone())
            .or_default();
        if !tombstones.iter().any(|current| current == causation) {
            tombstones.push(causation.clone());
        }
    }
    wait_for_removal(spawner, causation, timeout).await
}

async fn wait_for_removal(
    spawner: &ProductionWorkflowSpawner,
    causation: &WorkflowPermissionCausation,
    timeout: Duration,
) -> WorkflowCleanupStatus {
    if tokio::time::timeout(timeout, wait_until_removed(spawner, causation))
        .await
        .is_ok()
    {
        WorkflowCleanupStatus::Confirmed
    } else {
        WorkflowCleanupStatus::TimedOut
    }
}

async fn wait_until_removed(
    spawner: &ProductionWorkflowSpawner,
    causation: &WorkflowPermissionCausation,
) {
    loop {
        let notified = spawner.changed.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let orphan_free = spawner.cleanup_orphaned_preparation(causation).await;
        let done = {
            let owners = spawner.attempts.lock().await;
            let preparing = owners
                .preparing
                .get(&causation.attempt_id)
                .is_some_and(|owner| owner.causation == *causation);
            let owned = owners
                .by_attempt
                .get(&causation.attempt_id)
                .is_some_and(|owner| owner.causation == *causation);
            let pre_aborted = owners
                .pre_aborted
                .get(&causation.attempt_id)
                .is_some_and(|values| values.iter().any(|value| value == causation));
            orphan_free && !preparing && !owned && !pre_aborted
        };
        if done {
            return;
        }
        notified.await;
    }
}
