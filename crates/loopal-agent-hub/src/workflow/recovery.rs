use std::collections::{HashMap, HashSet};

use loopal_protocol::{
    WorkflowEventPayload, WorkflowRequestId, WorkflowRequestLedger, WorkflowRequestRecord,
    WorkflowRunSnapshot, WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
};
use loopal_storage::WorkflowJournalReplay;

use super::transition::apply_event;
use super::validation::validate_output_contract;
use super::{WorkflowCoordinatorError, WorkflowOwner};

#[path = "recovery/requests.rs"]
mod requests;

/// Authenticated facts supplied by the Hub registration boundary when an
/// already-running worker reconnects after coordinator recovery.
#[derive(Clone, Debug)]
pub(crate) struct WorkflowAttemptReconnect {
    pub(crate) causation: loopal_protocol::WorkflowPermissionCausation,
    pub(crate) capability: loopal_protocol::WorkflowAttemptCapability,
    pub(crate) execution: crate::types::AgentExecutionRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkflowAttemptReconnectResponse {
    pub(crate) execution: crate::types::AgentExecutionRef,
    pub(crate) attempt_state: loopal_protocol::WorkflowAttemptState,
}

pub(crate) struct RecoveredOwner {
    pub(crate) runs: Vec<WorkflowRunSnapshot>,
    pub(crate) requests: WorkflowRequestLedger,
    pub(crate) delivery_intents: Vec<WorkflowTerminalNotification>,
    pub(crate) acked_deliveries: HashSet<WorkflowTerminalDeliveryId>,
}

type RecoveredRun = (
    WorkflowRunSnapshot,
    Vec<WorkflowRequestRecord>,
    Vec<WorkflowTerminalNotification>,
    Vec<WorkflowTerminalDeliveryId>,
);

pub(super) fn recover_owner(
    owner: &WorkflowOwner,
    replays: Vec<WorkflowJournalReplay>,
) -> Result<RecoveredOwner, WorkflowCoordinatorError> {
    let mut runs = Vec::with_capacity(replays.len());
    let mut records = HashMap::new();
    let mut run_ids = HashSet::new();
    let mut acked_deliveries = HashSet::new();
    let mut delivery_intents = Vec::new();
    for replay in replays {
        let (snapshot, recovered, intents, delivery_acks) = recover_run(owner, replay)?;
        if !run_ids.insert(snapshot.id.clone()) {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
        for record in recovered {
            record_unique(&mut records, record)?;
        }
        for delivery_id in delivery_acks {
            acked_deliveries.insert(delivery_id);
        }
        delivery_intents.extend(intents);
        runs.push(snapshot);
    }
    let mut requests = WorkflowRequestLedger::default();
    let mut records: Vec<_> = records.into_values().collect();
    records.sort_by(|left, right| left.request_id.cmp(&right.request_id));
    for record in records {
        requests
            .record(record)
            .map_err(|_| WorkflowCoordinatorError::RecoveryInvalid)?;
    }
    runs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(RecoveredOwner {
        runs,
        requests,
        delivery_intents,
        acked_deliveries,
    })
}

fn recover_run(
    owner: &WorkflowOwner,
    replay: WorkflowJournalReplay,
) -> Result<RecoveredRun, WorkflowCoordinatorError> {
    if replay.torn_tail.is_some() {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    let init = replay
        .init
        .ok_or(WorkflowCoordinatorError::RecoveryInvalid)?;
    if init.snapshot.root_agent != owner.root_agent
        || init.events.len() != 1
        || !matches!(init.events[0].payload, WorkflowEventPayload::SpecValidated)
    {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    validate_output_contract(&init.snapshot.spec.output_contract)?;
    let mut snapshot = apply_event(&init.snapshot, &init.events[0])?;
    let start = init
        .request
        .ok_or(WorkflowCoordinatorError::RecoveryInvalid)?;
    requests::validate_start(&start, &init.snapshot, &snapshot)?;
    let mut requests = vec![start];
    for commit in replay.commits {
        if commit.run_id != snapshot.id {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
        let event_count = commit.events.len();
        let is_cancel_event = matches!(
            commit.events.as_slice(),
            [loopal_protocol::WorkflowEvent {
                payload: WorkflowEventPayload::CancelRequested { .. },
                ..
            }]
        );
        if event_count == 0 && commit.request.is_none() {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
        for event in commit.events {
            snapshot = apply_event(&snapshot, &event)?;
        }
        if let Some(request) = commit.request {
            match request.operation.as_str() {
                "get" if event_count == 0 => requests::validate_get(&request, &snapshot)?,
                "cancel" if event_count == 0 || is_cancel_event => {
                    requests::validate_cancel(&request, &snapshot, event_count != 0)?
                }
                _ => return Err(WorkflowCoordinatorError::RecoveryInvalid),
            }
            requests.push(request);
        }
    }
    for delivery_id in &replay.delivery_acks {
        if delivery_id.session_id != owner.session_id
            || delivery_id.run_id != snapshot.id
            || delivery_id.terminal_revision != snapshot.revision
            || !snapshot.state.is_terminal()
        {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
    }
    for notification in &replay.delivery_intents {
        let delivery_id = &notification.delivery_id;
        if notification.validate().is_err()
            || delivery_id.session_id != owner.session_id
            || delivery_id.run_id != snapshot.id
            || delivery_id.terminal_revision != snapshot.revision
            || notification.state != snapshot.state
            || !snapshot.state.is_terminal()
        {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
    }
    if replay.delivery_intents.len() > 1
        || replay.delivery_acks.iter().any(|ack| {
            !replay
                .delivery_intents
                .iter()
                .any(|intent| intent.delivery_id == *ack)
        })
    {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    Ok((
        snapshot,
        requests,
        replay.delivery_intents,
        replay.delivery_acks,
    ))
}

fn record_unique(
    records: &mut HashMap<WorkflowRequestId, WorkflowRequestRecord>,
    record: WorkflowRequestRecord,
) -> Result<(), WorkflowCoordinatorError> {
    match records.get(&record.request_id) {
        Some(existing) if existing != &record => Err(WorkflowCoordinatorError::RecoveryInvalid),
        Some(_) => Ok(()),
        None => {
            records.insert(record.request_id.clone(), record);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
