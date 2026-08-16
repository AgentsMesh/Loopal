use loopal_protocol::{WorkflowEvent, WorkflowEventPayload, WorkflowRunId, WorkflowRunSnapshot};

use super::super::WorkflowCoordinator;
use crate::workflow::actor::admission::await_append;
use crate::workflow::transition::apply_payload;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) async fn payload(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run: &WorkflowRunSnapshot,
    payload: WorkflowEventPayload,
    occurred_at_unix_ms: u64,
) -> Result<WorkflowRunSnapshot, WorkflowCoordinatorError> {
    let (event, next) = apply_payload(run, payload, occurred_at_unix_ms)?;
    events(coordinator, owner, &run.id, vec![event], next).await
}

async fn events(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run_id: &WorkflowRunId,
    events: Vec<WorkflowEvent>,
    next: WorkflowRunSnapshot,
) -> Result<WorkflowRunSnapshot, WorkflowCoordinatorError> {
    let journal = coordinator.journal.clone();
    let append_owner = owner.clone();
    let append_run_id = run_id.clone();
    let append = tokio::task::spawn_blocking(move || {
        journal.append_commit(&append_owner, &append_run_id, events, None)
    });
    if let Err(error) = await_append(append).await {
        coordinator.poison_owner(owner.clone());
        return Err(error);
    }
    if next.state.is_terminal() {
        crate::workflow::terminal_delivery::prepare_intent(coordinator, owner, &next).await?;
    }
    coordinator.state.commit_transition(owner, next.clone())?;
    coordinator.publish_revision(owner, &next);
    Ok(next)
}
