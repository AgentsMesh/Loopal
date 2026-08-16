mod hub_sink;
mod intent;
pub(in crate::workflow) mod payload;

pub(crate) use hub_sink::HubWorkflowTerminalSink;
pub(super) use intent::{activate, prepare as prepare_intent};

use std::sync::Arc;

use loopal_protocol::{
    WorkflowRunSnapshot, WorkflowTerminalDeliveryId, WorkflowTerminalDisposition,
    WorkflowTerminalNotification,
};

use super::actor::WorkflowCoordinator;
use super::command::WorkflowCommand;
use super::journal::WorkflowJournalDeliveryAckOutcome;
use super::{WorkflowCoordinatorError, WorkflowOwner};

#[async_trait::async_trait]
pub(crate) trait WorkflowTerminalSink: Send + Sync + 'static {
    async fn deliver(
        &self,
        owner: &WorkflowOwner,
        notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowTerminalDisposition, String>;
}

pub(crate) struct UnavailableWorkflowTerminalSink;

#[async_trait::async_trait]
impl WorkflowTerminalSink for UnavailableWorkflowTerminalSink {
    async fn deliver(
        &self,
        _owner: &WorkflowOwner,
        _notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowTerminalDisposition, String> {
        Err("workflow terminal sink is unavailable".into())
    }
}

pub(super) fn schedule(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    snapshot: &WorkflowRunSnapshot,
) {
    if !coordinator.terminal_delivery_owners.contains(owner) || !snapshot.state.is_terminal() {
        return;
    }
    let delivery_id = WorkflowTerminalDeliveryId::new(
        owner.session_id.clone(),
        snapshot.id.clone(),
        snapshot.revision,
    );
    if coordinator.state.is_delivery_acked(&delivery_id)
        || !coordinator.terminal_deliveries.insert(delivery_id.clone())
    {
        return;
    }
    let Some(notification) = coordinator
        .terminal_delivery_payloads
        .get(&delivery_id)
        .cloned()
    else {
        coordinator.terminal_deliveries.remove(&delivery_id);
        coordinator.terminal_delivery_failure = Some(WorkflowCoordinatorError::RecoveryInvalid);
        coordinator.poison_owner(owner.clone());
        tracing::error!(run_id = %snapshot.id, "durable workflow terminal intent is missing");
        return;
    };
    spawn(coordinator, owner.clone(), notification);
}

pub(super) fn retry_owner(coordinator: &mut WorkflowCoordinator, owner: &WorkflowOwner) {
    for snapshot in coordinator.state.owner_snapshots(owner) {
        schedule(coordinator, owner, &snapshot);
    }
}

pub(super) fn retry_all(coordinator: &mut WorkflowCoordinator) {
    for (owner, snapshot) in coordinator.state.scheduler_runs() {
        schedule(coordinator, &owner, &snapshot);
    }
}

fn spawn(
    coordinator: &WorkflowCoordinator,
    owner: WorkflowOwner,
    notification: WorkflowTerminalNotification,
) {
    let sink = Arc::clone(&coordinator.terminal_sink);
    let callbacks = coordinator.callbacks.clone();
    tokio::spawn(async move {
        let delivery_id = notification.delivery_id.clone();
        let delivery_owner = owner.clone();
        let delivery =
            tokio::spawn(async move { sink.deliver(&delivery_owner, notification).await });
        let (result, task_panicked) = match delivery.await {
            Ok(result) => (result, false),
            Err(error) => (
                Err(format!("workflow terminal delivery task failed: {error}")),
                true,
            ),
        };
        if let Some(callbacks) = callbacks.upgrade() {
            let _ = callbacks
                .send(WorkflowCommand::TerminalDeliveryResolved {
                    owner,
                    delivery_id,
                    result,
                    task_panicked,
                })
                .await;
        }
    });
}

pub(super) async fn resolved(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    delivery_id: WorkflowTerminalDeliveryId,
    result: Result<WorkflowTerminalDisposition, String>,
    task_panicked: bool,
) -> Result<(), WorkflowCoordinatorError> {
    if !coordinator.terminal_deliveries.remove(&delivery_id) {
        return Ok(());
    }
    if task_panicked {
        let error = WorkflowCoordinatorError::Unavailable;
        coordinator.terminal_delivery_failure = Some(error.clone());
        coordinator.poison_owner(owner);
        return Err(error);
    }
    match result {
        Ok(WorkflowTerminalDisposition::Applied)
        | Ok(WorkflowTerminalDisposition::AlreadyApplied) => {}
        Ok(WorkflowTerminalDisposition::Queued)
        | Ok(WorkflowTerminalDisposition::Retryable { .. })
        | Err(_) => return Ok(()),
        Ok(WorkflowTerminalDisposition::Rejected { .. }) => {
            let error = WorkflowCoordinatorError::RecoveryConflict;
            coordinator.terminal_delivery_failure = Some(error.clone());
            coordinator.poison_owner(owner);
            return Err(error);
        }
    }
    let journal = Arc::clone(&coordinator.journal);
    let append_owner = owner.clone();
    let append_id = delivery_id.clone();
    let append = match tokio::task::spawn_blocking(move || {
        journal.append_delivery_ack(&append_owner, &append_id)
    })
    .await
    {
        Ok(append) => append,
        Err(_) => {
            let error = WorkflowCoordinatorError::JournalUnavailable;
            coordinator.terminal_delivery_failure = Some(error.clone());
            coordinator.poison_owner(owner);
            return Err(error);
        }
    };
    match append {
        Ok(WorkflowJournalDeliveryAckOutcome::Appended)
        | Ok(WorkflowJournalDeliveryAckOutcome::AlreadyPresent) => {
            coordinator.terminal_delivery_payloads.remove(&delivery_id);
            coordinator.state.commit_delivery_ack(delivery_id);
            Ok(())
        }
        Err(error) => {
            coordinator.terminal_delivery_failure = Some(error.clone());
            coordinator.poison_owner(owner);
            Err(error)
        }
    }
}
