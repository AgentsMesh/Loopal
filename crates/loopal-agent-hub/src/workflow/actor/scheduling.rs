mod callbacks;
mod commit;
mod dispatch;
mod drainage;
mod recovery;
mod resume;
mod stop;
mod tick;

#[cfg(test)]
#[path = "scheduling/coverage_tests/mod.rs"]
mod coverage_tests;

use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowRunId, WorkflowRunSnapshot,
};

use super::WorkflowCoordinator;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

impl WorkflowCoordinator {
    pub(super) async fn drain_scheduler(&mut self) -> Result<(), WorkflowCoordinatorError> {
        drainage::run(self).await
    }

    pub(super) async fn admit_schedule(
        &mut self,
        owner: WorkflowOwner,
        run_id: WorkflowRunId,
    ) -> Result<(), WorkflowCoordinatorError> {
        dispatch::admit(self, owner, run_id).await
    }

    pub(super) async fn reconcile_recovered(
        &mut self,
        owner: &WorkflowOwner,
        now_unix_ms: u64,
    ) -> Result<(), WorkflowCoordinatorError> {
        recovery::reconcile(self, owner, now_unix_ms).await
    }

    pub(super) async fn reconnect_attempt(
        &mut self,
        owner: WorkflowOwner,
        request: crate::workflow::recovery::WorkflowAttemptReconnect,
    ) -> Result<crate::workflow::recovery::WorkflowAttemptReconnectResponse, WorkflowCoordinatorError>
    {
        recovery::adopt(self, owner, request).await
    }

    pub(super) async fn worker_handshake(
        &mut self,
        owner: WorkflowOwner,
        request: crate::workflow::recovery::WorkflowAttemptReconnect,
    ) -> Result<loopal_protocol::WorkflowWorkerHandshakeResponse, WorkflowCoordinatorError> {
        recovery::handshake(self, owner, request).await
    }

    pub(super) fn quarantine_owner(&mut self, owner: &WorkflowOwner) {
        stop::quarantine_owner(self, owner);
    }

    pub(super) fn contain_execution(&self, execution: crate::types::AgentExecutionRef) {
        stop::contain_execution(self, execution);
    }

    pub(super) async fn handle_prepared(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        prepared: crate::workflow::scheduler::WorkflowPreparedDelivery,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::prepared(self, owner, key, prepared).await
    }

    pub(super) fn handle_preparation_timed_out(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        failure: crate::workflow::scheduler::WorkflowSpawnFailure,
    ) {
        callbacks::preparation_timed_out(self, owner, key, failure);
    }

    pub(super) async fn handle_preparation_aborted(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        status: crate::workflow::scheduler::WorkflowCleanupStatus,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::preparation_aborted(self, owner, key, status).await
    }

    pub(super) async fn handle_preparation_abort_settled(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::preparation_abort_settled(self, owner, key).await
    }

    pub(super) async fn handle_preparation_delivery_finished(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::preparation_delivery_finished(self, owner, key).await
    }

    pub(super) async fn handle_late_preparation_shutdown(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        status: crate::workflow::scheduler::WorkflowCleanupStatus,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::late_preparation_shutdown(self, owner, key, execution, status).await
    }

    pub(super) async fn handle_activated(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        result: Result<(), crate::workflow::scheduler::WorkflowActivationFailure>,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::activated(self, owner, key, execution, result).await
    }

    pub(super) async fn handle_finished(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        outcome: crate::workflow::scheduler::WorkflowWorkerOutcome,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::finished(self, owner, key, execution, outcome).await
    }

    pub(super) async fn handle_outcome_lost(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
    ) -> Result<(), WorkflowCoordinatorError> {
        callbacks::outcome_lost(self, owner, key, execution).await
    }

    pub(super) async fn handle_stopped(
        &mut self,
        owner: WorkflowOwner,
        key: crate::workflow::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        status: crate::workflow::scheduler::WorkflowCleanupStatus,
    ) -> Result<(), WorkflowCoordinatorError> {
        stop::stopped(self, owner, key, execution, status).await
    }

    pub(super) async fn admit_cancel(
        &mut self,
        owner: WorkflowOwner,
        request: WorkflowCancelRequest,
    ) -> Result<WorkflowCancelResponse, WorkflowCoordinatorError> {
        crate::workflow::cancel::admit(self, owner, request).await
    }

    pub(in crate::workflow) fn scheduler_snapshot(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunSnapshot, WorkflowCoordinatorError> {
        self.state
            .owned_snapshot(owner, run_id)
            .ok_or(WorkflowCoordinatorError::InvalidRunId)
    }

    pub(in crate::workflow) fn begin_cancel_effects(
        &mut self,
        owner: WorkflowOwner,
        run_id: WorkflowRunId,
        reason: String,
    ) {
        stop::begin_cancel_effects(self, owner, run_id, reason);
    }
}

pub(in crate::workflow) fn bound_reason(reason: String) -> String {
    stop::bound_reason(reason)
}
