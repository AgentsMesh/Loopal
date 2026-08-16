use loopal_protocol::{
    WorkflowEvent, WorkflowEventPayload, WorkflowRequestDecision, WorkflowRequestRecord,
    WorkflowRunSnapshot, WorkflowRunSummary, WorkflowStartRequest, WorkflowStartResponse,
    validate_workflow_spec,
};

use super::state::WorkflowActorState;
use super::transition::apply_payload;
use super::validation::validate_output_contract;
use super::{WorkflowClock, WorkflowCoordinatorError, WorkflowIdSource, WorkflowOwner};

pub(super) enum StartDecision {
    Replay(WorkflowStartResponse),
    New(Box<PreparedStart>),
}

pub(super) struct PreparedStart {
    pub(super) owner: WorkflowOwner,
    pub(super) planned: WorkflowRunSnapshot,
    pub(super) event: WorkflowEvent,
    pub(super) snapshot: WorkflowRunSnapshot,
    pub(super) request: WorkflowRequestRecord,
    pub(super) response: WorkflowStartResponse,
    pub(super) next_ledger: loopal_protocol::WorkflowRequestLedger,
}

impl WorkflowActorState {
    pub(super) fn prepare_start(
        &self,
        owner: WorkflowOwner,
        request: WorkflowStartRequest,
        clock: &dyn WorkflowClock,
        ids: &dyn WorkflowIdSource,
    ) -> Result<StartDecision, WorkflowCoordinatorError> {
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
            ledger.decide(&request.request_id, "start", &payload)?
        {
            return serde_json::from_value(response.clone())
                .map(StartDecision::Replay)
                .map_err(|_| WorkflowCoordinatorError::RecoveryInvalid);
        }

        validate_workflow_spec(&request.spec)?;
        validate_output_contract(&request.spec.output_contract)?;
        let run_id = ids.next_run_id();
        if !run_id.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidGeneratedRunId(run_id));
        }
        if self.runs.contains_key(&run_id) {
            return Err(WorkflowCoordinatorError::RunIdCollision(run_id));
        }
        let created_at_unix_ms = clock.now_unix_ms();
        let planned = WorkflowRunSnapshot::planned(
            run_id,
            owner.root_agent.clone(),
            request.spec,
            created_at_unix_ms,
        );
        let (event, snapshot) = apply_payload(
            &planned,
            WorkflowEventPayload::SpecValidated,
            clock.now_unix_ms().max(created_at_unix_ms),
        )?;
        let response = WorkflowStartResponse {
            summary: WorkflowRunSummary::from(&snapshot),
        };
        let record = WorkflowRequestRecord {
            request_id: request.request_id,
            operation: "start".into(),
            payload,
            response: encode(&response)?,
        };
        let mut next_ledger = ledger;
        next_ledger.record(record.clone())?;
        Ok(StartDecision::New(Box::new(PreparedStart {
            owner,
            planned,
            event,
            snapshot,
            request: record,
            response,
            next_ledger,
        })))
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, WorkflowCoordinatorError> {
    serde_json::to_value(value)
        .map_err(|error| WorkflowCoordinatorError::Encoding(error.to_string()))
}
