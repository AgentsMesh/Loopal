use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowEvent, WorkflowEventPayload,
    WorkflowRequestDecision, WorkflowRequestRecord, WorkflowRunSnapshot, WorkflowRunSummary,
};

use super::actor::WorkflowCoordinator;
use super::actor::admission::await_append;
use super::transition::apply_payload;
use super::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) async fn admit(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    request: WorkflowCancelRequest,
) -> Result<WorkflowCancelResponse, WorkflowCoordinatorError> {
    validate(coordinator, &owner, &request).await?;
    let payload = encode(&request)?;
    let ledger = coordinator
        .state
        .requests
        .get(&owner)
        .cloned()
        .unwrap_or_default();
    if let WorkflowRequestDecision::Replay(response) =
        ledger.decide(&request.request_id, "cancel", &payload)?
    {
        return decode(response);
    }
    let current = coordinator.scheduler_snapshot(&owner, &request.run_id)?;
    let already_terminal = current.state.is_terminal();
    let (event, next) = prepare_transition(coordinator, &current, &request)?;
    let response = WorkflowCancelResponse {
        summary: WorkflowRunSummary::from(&next),
        already_terminal,
    };
    let effect_reason = event_reason(&request);
    let record = WorkflowRequestRecord {
        request_id: request.request_id,
        operation: "cancel".into(),
        payload,
        response: encode(&response)?,
    };
    let mut next_ledger = ledger;
    next_ledger.record(record.clone())?;
    append(coordinator, &owner, &current, event.clone(), record).await?;
    if event.is_some() && next.state.is_terminal() {
        super::terminal_delivery::prepare_intent(coordinator, &owner, &next).await?;
    }
    coordinator
        .state
        .requests
        .insert(owner.clone(), next_ledger);
    if event.is_some() {
        coordinator.state.commit_transition(&owner, next.clone())?;
        coordinator.publish_revision(&owner, &next);
        coordinator.begin_cancel_effects(owner, current.id, effect_reason);
    }
    Ok(response)
}

async fn validate(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    request: &WorkflowCancelRequest,
) -> Result<(), WorkflowCoordinatorError> {
    if coordinator.mode == super::WorkflowCoordinatorMode::Disabled {
        return Err(WorkflowCoordinatorError::Disabled);
    }
    if !owner.is_valid() {
        return Err(WorkflowCoordinatorError::InvalidOwner);
    }
    if !request.run_id.is_valid() {
        return Err(WorkflowCoordinatorError::InvalidRunId);
    }
    if !coordinator.state.is_recovered(owner) {
        coordinator.recover_owner(owner.clone()).await?;
    }
    Ok(())
}

fn prepare_transition(
    coordinator: &WorkflowCoordinator,
    current: &WorkflowRunSnapshot,
    request: &WorkflowCancelRequest,
) -> Result<(Option<WorkflowEvent>, WorkflowRunSnapshot), WorkflowCoordinatorError> {
    if current.state.is_terminal() || current.state == loopal_protocol::WorkflowRunState::Cancelling
    {
        return Ok((None, current.clone()));
    }
    let (event, next) = apply_payload(
        current,
        WorkflowEventPayload::CancelRequested {
            reason: Some(event_reason(request)),
        },
        coordinator.clock.now_unix_ms(),
    )?;
    Ok((Some(event), next))
}

async fn append(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    current: &WorkflowRunSnapshot,
    event: Option<WorkflowEvent>,
    record: WorkflowRequestRecord,
) -> Result<(), WorkflowCoordinatorError> {
    let journal = coordinator.journal.clone();
    let journal_owner = owner.clone();
    let run_id = current.id.clone();
    let append = tokio::task::spawn_blocking(move || {
        journal.append_commit(
            &journal_owner,
            &run_id,
            event.into_iter().collect(),
            Some(record),
        )
    });
    if let Err(error) = await_append(append).await {
        coordinator.poison_owner(owner.clone());
        return Err(error);
    }
    Ok(())
}

pub(super) fn event_reason(request: &WorkflowCancelRequest) -> String {
    super::actor::scheduling::bound_reason(
        request
            .reason
            .clone()
            .unwrap_or_else(|| "workflow cancelled".into()),
    )
}

fn encode<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, WorkflowCoordinatorError> {
    serde_json::to_value(value)
        .map_err(|error| WorkflowCoordinatorError::Encoding(error.to_string()))
}

fn decode<T: serde::de::DeserializeOwned>(
    value: &serde_json::Value,
) -> Result<T, WorkflowCoordinatorError> {
    serde_json::from_value(value.clone()).map_err(|_| WorkflowCoordinatorError::RecoveryInvalid)
}
