use super::super::WorkflowCoordinator;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

impl WorkflowCoordinator {
    /// Re-open scheduling only after the root session has completed its
    /// startup handshake. The scan is intentionally idempotent: a run with
    /// an unresolved attempt or in-memory custody is left for recovery/stop
    /// handling, while quiescent runs are admitted in deterministic id order.
    pub(in crate::workflow::actor) async fn resume_owner(
        &mut self,
        owner: WorkflowOwner,
    ) -> Result<(), WorkflowCoordinatorError> {
        if !self.mode.executes() {
            return Err(WorkflowCoordinatorError::Disabled);
        }
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if self.state.is_poisoned(&owner) {
            return Err(WorkflowCoordinatorError::OwnerPoisoned);
        }
        if !self.state.is_recovered(&owner) {
            return Err(WorkflowCoordinatorError::RecoveryRequired);
        }

        let runs = self.state.owner_snapshots(&owner);
        for run in runs {
            if !matches!(
                run.state,
                loopal_protocol::WorkflowRunState::Validated
                    | loopal_protocol::WorkflowRunState::Running
            ) || run
                .attempts
                .iter()
                .any(|attempt| !attempt.state.is_terminal())
                || self
                    .pending
                    .values()
                    .any(|attempt| attempt.owner == owner && attempt.key.run_id == run.id)
                || self
                    .active
                    .values()
                    .any(|attempt| attempt.owner == owner && attempt.key.run_id == run.id)
            {
                continue;
            }
            if run.state == loopal_protocol::WorkflowRunState::Running
                && !run
                    .nodes
                    .iter()
                    .any(|node| node.state == loopal_protocol::WorkflowNodeState::Ready)
            {
                continue;
            }
            match self.admit_schedule(owner.clone(), run.id.clone()).await {
                Ok(()) | Err(WorkflowCoordinatorError::RunDeadlineExceeded) => {}
                Err(error) => {
                    self.poison_owner(owner.clone());
                    return Err(error);
                }
            }
        }
        self.resumed_owners.insert(owner);
        Ok(())
    }
}
