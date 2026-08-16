use loopal_protocol::{
    WorkflowGetRequest, WorkflowGetResponse, WorkflowRequestDecision, WorkflowRequestLedger,
    WorkflowRequestRecord,
};

use super::state::WorkflowActorState;
use super::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) enum GetDecision {
    Replay(Box<WorkflowGetResponse>),
    New(Box<PreparedGet>),
}

pub(super) struct PreparedGet {
    pub(super) owner: WorkflowOwner,
    pub(super) record: WorkflowRequestRecord,
    pub(super) response: WorkflowGetResponse,
    pub(super) next_ledger: WorkflowRequestLedger,
    pub(super) journaled: bool,
}

impl WorkflowActorState {
    pub(super) fn prepare_get(
        &self,
        owner: WorkflowOwner,
        request: WorkflowGetRequest,
    ) -> Result<GetDecision, WorkflowCoordinatorError> {
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if self.is_poisoned(&owner) {
            return Err(WorkflowCoordinatorError::OwnerPoisoned);
        }
        if !self.is_recovered(&owner) {
            return Err(WorkflowCoordinatorError::RecoveryRequired);
        }
        let payload = encode(&request)?;
        let ledger = self.requests.get(&owner).cloned().unwrap_or_default();
        if let WorkflowRequestDecision::Replay(response) =
            ledger.decide(&request.request_id, "get", &payload)?
        {
            return serde_json::from_value(response.clone())
                .map(Box::new)
                .map(GetDecision::Replay)
                .map_err(|_| WorkflowCoordinatorError::RecoveryInvalid);
        }
        if !request.run_id.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidRunId);
        }
        let run = self.owned_snapshot(&owner, &request.run_id);
        let journaled = run.is_some();
        let response = WorkflowGetResponse { run };
        let record = WorkflowRequestRecord {
            request_id: request.request_id,
            operation: "get".into(),
            payload,
            response: encode(&response)?,
        };
        let mut next_ledger = ledger;
        next_ledger.record(record.clone())?;
        Ok(GetDecision::New(Box::new(PreparedGet {
            owner,
            record,
            response,
            next_ledger,
            journaled,
        })))
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, WorkflowCoordinatorError> {
    serde_json::to_value(value)
        .map_err(|error| WorkflowCoordinatorError::Encoding(error.to_string()))
}
