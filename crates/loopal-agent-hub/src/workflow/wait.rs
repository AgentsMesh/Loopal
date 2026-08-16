use std::collections::HashMap;
use std::time::Duration;

use loopal_protocol::{
    MAX_WORKFLOW_WAIT_MS, WorkflowRequestError, WorkflowRunId, WorkflowRunSnapshot,
    WorkflowWaitRequest, WorkflowWaitResponse, WorkflowWaitStatus,
};
use tokio::sync::watch;

use super::actor::WorkflowCoordinator;
use super::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) type RevisionSenders =
    HashMap<(WorkflowOwner, WorkflowRunId), watch::Sender<WorkflowRunSnapshot>>;

pub(super) fn validate_request(
    request: &WorkflowWaitRequest,
) -> Result<(), WorkflowCoordinatorError> {
    if !request.request_id.is_valid() {
        return Err(WorkflowCoordinatorError::Request(
            WorkflowRequestError::InvalidRequestId,
        ));
    }
    if request.timeout_ms > MAX_WORKFLOW_WAIT_MS {
        return Err(WorkflowCoordinatorError::WaitTimeoutExceeded);
    }
    Ok(())
}

impl WorkflowCoordinator {
    pub(super) async fn subscribe(
        &mut self,
        owner: WorkflowOwner,
        run_id: WorkflowRunId,
    ) -> Result<Option<watch::Receiver<WorkflowRunSnapshot>>, WorkflowCoordinatorError> {
        if self.mode == super::WorkflowCoordinatorMode::Disabled {
            return Err(WorkflowCoordinatorError::Disabled);
        }
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if !run_id.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidRunId);
        }
        if self.state.is_poisoned(&owner) {
            return Err(WorkflowCoordinatorError::OwnerPoisoned);
        }
        if !self.state.is_recovered(&owner) {
            self.recover_owner(owner.clone()).await?;
        }
        let Some(snapshot) = self.state.owned_snapshot(&owner, &run_id) else {
            return Ok(None);
        };
        let key = (owner, run_id);
        let sender = self.revisions.entry(key).or_insert_with(|| {
            let (sender, _) = watch::channel(snapshot.clone());
            sender
        });
        sender.send_replace(snapshot);
        Ok(Some(sender.subscribe()))
    }
}

pub(super) fn publish(
    senders: &mut RevisionSenders,
    owner: &WorkflowOwner,
    snapshot: &WorkflowRunSnapshot,
) {
    if let Some(sender) = senders.get_mut(&(owner.clone(), snapshot.id.clone())) {
        sender.send_replace(snapshot.clone());
    }
}

pub(super) async fn wait(
    mut revision: Option<watch::Receiver<WorkflowRunSnapshot>>,
    request: WorkflowWaitRequest,
) -> Result<WorkflowWaitResponse, WorkflowCoordinatorError> {
    let Some(ref mut revision) = revision else {
        return Ok(response(WorkflowWaitStatus::NotFound, None));
    };
    if let Some(response) = changed(revision.borrow().clone(), request.after_revision) {
        return Ok(response);
    }
    let duration = Duration::from_millis(request.timeout_ms);
    match tokio::time::timeout(duration, revision.changed()).await {
        Ok(Ok(())) => Ok(changed(revision.borrow().clone(), request.after_revision)
            .unwrap_or_else(|| response(WorkflowWaitStatus::TimedOut, None))),
        Ok(Err(_)) => Err(WorkflowCoordinatorError::Unavailable),
        Err(_) => Ok(response(WorkflowWaitStatus::TimedOut, None)),
    }
}

fn changed(snapshot: WorkflowRunSnapshot, after_revision: u64) -> Option<WorkflowWaitResponse> {
    if snapshot.state.is_terminal() {
        Some(response(WorkflowWaitStatus::Terminal, Some(snapshot)))
    } else if snapshot.revision > after_revision {
        Some(response(WorkflowWaitStatus::Changed, Some(snapshot)))
    } else {
        None
    }
}

fn response(status: WorkflowWaitStatus, run: Option<WorkflowRunSnapshot>) -> WorkflowWaitResponse {
    WorkflowWaitResponse { status, run }
}
