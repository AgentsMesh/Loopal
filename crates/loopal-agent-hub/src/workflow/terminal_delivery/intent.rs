use std::sync::Arc;

use loopal_protocol::{
    WorkflowRunSnapshot, WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
};

use super::super::actor::WorkflowCoordinator;
use super::super::journal::WorkflowJournalDeliveryIntentOutcome;
use super::super::{WorkflowCoordinatorError, WorkflowOwner};

pub(in crate::workflow) async fn activate(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
) -> Result<(), WorkflowCoordinatorError> {
    if !coordinator.state.is_recovered(&owner) {
        return Err(WorkflowCoordinatorError::RecoveryRequired);
    }
    if let Some(error) = coordinator.terminal_delivery_failure.clone() {
        return Err(error);
    }
    if coordinator.state.is_poisoned(&owner) {
        return Err(WorkflowCoordinatorError::OwnerPoisoned);
    }
    for snapshot in coordinator.state.owner_snapshots(&owner) {
        if needs_intent(coordinator, &owner, &snapshot) {
            prepare(coordinator, &owner, &snapshot).await?;
        }
    }
    coordinator.terminal_delivery_owners.insert(owner.clone());
    super::retry_owner(coordinator, &owner);
    Ok(())
}

fn needs_intent(
    coordinator: &WorkflowCoordinator,
    owner: &WorkflowOwner,
    snapshot: &WorkflowRunSnapshot,
) -> bool {
    snapshot.state.is_terminal()
        && !coordinator
            .state
            .is_delivery_acked(&WorkflowTerminalDeliveryId::new(
                owner.session_id.clone(),
                snapshot.id.clone(),
                snapshot.revision,
            ))
}

pub(in crate::workflow) async fn prepare(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    snapshot: &WorkflowRunSnapshot,
) -> Result<(), WorkflowCoordinatorError> {
    if coordinator.state.is_poisoned(owner) {
        return Err(WorkflowCoordinatorError::OwnerPoisoned);
    }
    let delivery_id = WorkflowTerminalDeliveryId::new(
        owner.session_id.clone(),
        snapshot.id.clone(),
        snapshot.revision,
    );
    if coordinator
        .terminal_delivery_payloads
        .contains_key(&delivery_id)
    {
        return Ok(());
    }
    let notification =
        match super::payload::from_snapshot(owner, snapshot, &coordinator.redaction_seed) {
            Ok(notification) => notification,
            Err(error) => return fail(coordinator, owner, error),
        };
    let notification = append(coordinator, owner, notification).await?;
    coordinator
        .terminal_delivery_payloads
        .insert(delivery_id, notification);
    Ok(())
}

async fn append(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    notification: WorkflowTerminalNotification,
) -> Result<WorkflowTerminalNotification, WorkflowCoordinatorError> {
    if coordinator.state.is_poisoned(owner) {
        return Err(WorkflowCoordinatorError::OwnerPoisoned);
    }
    let journal = Arc::clone(&coordinator.journal);
    let append_owner = owner.clone();
    let append = tokio::task::spawn_blocking(move || {
        journal.append_delivery_intent(&append_owner, notification)
    })
    .await
    .map_err(|_| WorkflowCoordinatorError::JournalUnavailable);
    match append {
        Ok(Ok(WorkflowJournalDeliveryIntentOutcome::Appended(notification)))
        | Ok(Ok(WorkflowJournalDeliveryIntentOutcome::AlreadyPresent(notification))) => {
            Ok(notification)
        }
        Ok(Err(error)) | Err(error) => fail(coordinator, owner, error),
    }
}

fn fail<T>(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    error: WorkflowCoordinatorError,
) -> Result<T, WorkflowCoordinatorError> {
    coordinator.terminal_delivery_failure = Some(error.clone());
    coordinator.poison_owner(owner.clone());
    Err(error)
}
