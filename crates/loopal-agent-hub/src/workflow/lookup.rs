use loopal_protocol::{
    WorkflowRequestError, WorkflowStartLookupRequest, WorkflowStartLookupResponse,
    WorkflowStartResponse,
};

use super::actor::WorkflowCoordinator;
use super::state::WorkflowActorState;
use super::{WorkflowCoordinatorError, WorkflowOwner};

impl WorkflowActorState {
    fn lookup_start_request(
        &self,
        owner: &WorkflowOwner,
        request: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, WorkflowCoordinatorError> {
        if !request.request_id.is_valid() {
            return Err(WorkflowRequestError::InvalidRequestId.into());
        }
        let Some(record) = self.requests.get(owner).and_then(|ledger| {
            ledger
                .records()
                .iter()
                .find(|record| record.request_id == request.request_id)
        }) else {
            return Ok(WorkflowStartLookupResponse::NotFound);
        };
        if record.operation != "start" {
            return Ok(WorkflowStartLookupResponse::Conflict);
        }
        let response: WorkflowStartResponse = serde_json::from_value(record.response.clone())
            .map_err(|_| WorkflowCoordinatorError::RecoveryInvalid)?;
        if self.owned_snapshot(owner, &response.summary.id).is_none() {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
        Ok(WorkflowStartLookupResponse::Found { response })
    }
}

impl WorkflowCoordinator {
    pub(super) async fn admit_lookup_start(
        &mut self,
        owner: WorkflowOwner,
        request: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, WorkflowCoordinatorError> {
        if self.mode == super::actor::WorkflowCoordinatorMode::Disabled {
            return Err(WorkflowCoordinatorError::Disabled);
        }
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if self.state.is_poisoned(&owner) {
            return Err(WorkflowCoordinatorError::OwnerPoisoned);
        }
        if !self.state.is_recovered(&owner) {
            self.recover_owner(owner.clone()).await?;
        }
        self.state.lookup_start_request(&owner, request)
    }
}
