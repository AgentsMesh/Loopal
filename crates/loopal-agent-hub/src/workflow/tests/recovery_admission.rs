use super::super::recovery::RecoveredOwner;
use super::super::{WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::support::{coordinator, coordinator_with_journal, owner};

#[tokio::test]
async fn recovery_rejects_disabled_and_invalid_owner_before_storage() {
    let (disabled, disabled_task, _, _) = coordinator(WorkflowCoordinatorMode::Disabled, [], []);
    assert_eq!(
        disabled.recover(owner("session", "root")).await,
        Err(WorkflowCoordinatorError::Disabled)
    );
    drop(disabled);
    disabled_task.await.unwrap();

    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [], []);
    journal.push_recovery(Err(WorkflowCoordinatorError::RecoveryConflict));
    assert_eq!(
        handle.recover(owner("bad/session", "root")).await,
        Err(WorkflowCoordinatorError::InvalidOwner)
    );
    assert_eq!(
        handle.recover(owner("session", "root")).await,
        Err(WorkflowCoordinatorError::RecoveryConflict)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn successful_owner_recovery_is_cached() {
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [], []);
    let owner = owner("session", "root");
    journal.push_recovery(Ok(RecoveredOwner {
        runs: Vec::new(),
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    journal.push_recovery(Err(WorkflowCoordinatorError::RecoveryConflict));

    assert_eq!(handle.recover(owner.clone()).await.unwrap(), 0);
    assert_eq!(handle.recover(owner).await.unwrap(), 0);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn recovery_read_failures_are_retryable() {
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [], []);
    let owner = owner("session", "root");
    journal.push_recovery(Err(WorkflowCoordinatorError::JournalUnavailable));
    journal.push_recovery(Ok(RecoveredOwner {
        runs: Vec::new(),
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));

    assert_eq!(
        handle.recover(owner.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(handle.recover(owner).await.unwrap(), 0);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn recovery_task_failures_are_retryable() {
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [], []);
    let owner = owner("session", "root");
    journal.push_recovery_panic();
    journal.push_recovery(Ok(RecoveredOwner {
        runs: Vec::new(),
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));

    assert_eq!(
        handle.recover(owner.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(handle.recover(owner).await.unwrap(), 0);
    drop(handle);
    task.await.unwrap();
}
