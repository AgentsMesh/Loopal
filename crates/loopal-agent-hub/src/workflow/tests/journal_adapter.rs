use super::super::WorkflowCoordinatorError;
use super::super::journal::{
    StartJournalRecord, UnavailableWorkflowJournals, WorkflowJournalStorage,
};
use super::super::transition::apply_event;
use super::support::{owner, request, spec};
use loopal_protocol::{
    WorkflowRequestId, WorkflowRequestRecord, WorkflowRunId, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification,
};

#[test]
fn unavailable_storage_fails_closed_for_every_operation() {
    let storage = UnavailableWorkflowJournals;
    let owner = owner("session", "root");
    let run_id = WorkflowRunId::new("wrun_unavailable");
    let planned = loopal_protocol::WorkflowRunSnapshot::planned(
        run_id.clone(),
        owner.root_agent.clone(),
        spec(),
        10,
    );
    let event = loopal_protocol::WorkflowEvent {
        run_id: run_id.clone(),
        revision: 1,
        occurred_at_unix_ms: 11,
        payload: loopal_protocol::WorkflowEventPayload::SpecValidated,
    };
    let validated = apply_event(&planned, &event).unwrap();
    let start = request("wreq_start");
    let request = WorkflowRequestRecord {
        request_id: start.request_id.clone(),
        operation: "start".into(),
        payload: serde_json::to_value(start).unwrap(),
        response: serde_json::to_value(loopal_protocol::WorkflowStartResponse {
            summary: loopal_protocol::WorkflowRunSummary::from(&validated),
        })
        .unwrap(),
    };

    assert_eq!(
        storage.recover(&owner).map(|_| ()),
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        storage.append_start(StartJournalRecord {
            owner: owner.clone(),
            planned,
            event,
            request: request.clone(),
        }),
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        storage.append_request(
            &owner,
            &run_id,
            WorkflowRequestRecord {
                request_id: WorkflowRequestId::new("wreq_get"),
                operation: "get".into(),
                payload: serde_json::Value::Null,
                response: serde_json::Value::Null,
            },
        ),
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        storage.append_commit(&owner, &run_id, Vec::new(), None),
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        storage.append_delivery_intent(
            &owner,
            WorkflowTerminalNotification {
                delivery_id: WorkflowTerminalDeliveryId::new(
                    owner.session_id.clone(),
                    run_id.clone(),
                    validated.revision,
                ),
                state: loopal_protocol::WorkflowRunState::Cancelled,
                run_goal: "unavailable".into(),
                outcome: loopal_protocol::WorkflowTerminalOutcome::Cancelled {
                    reason: "cancelled".into(),
                },
                content: "cancelled".into(),
            },
        ),
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        storage.append_delivery_ack(
            &owner,
            &WorkflowTerminalDeliveryId::new(owner.session_id.clone(), run_id, validated.revision,),
        ),
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
}
