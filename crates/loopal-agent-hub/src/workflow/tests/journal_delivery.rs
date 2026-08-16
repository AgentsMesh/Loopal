use std::sync::Arc;

use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{WorkflowRunId, WorkflowTerminalDeliveryId, WorkflowTerminalNotification};
use loopal_storage::SessionStore;

use super::super::journal::{
    SessionWorkflowJournals, StartJournalRecord, WorkflowJournalDeliveryAckOutcome,
    WorkflowJournalDeliveryIntentOutcome, WorkflowJournalStorage,
};
use super::super::transition::apply_event;
use super::support::{owner, request, spec};

#[test]
fn production_adapter_delivery_ack_is_durably_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let sessions = Arc::new(SessionStore::with_base_dir(temp.path().to_path_buf()));
    let storage = SessionWorkflowJournals::new(sessions.clone());
    let owner = owner("session-ack", "root");
    let run_id = WorkflowRunId::new("wrun_ack");
    let mut planned = loopal_protocol::WorkflowRunSnapshot::planned(
        run_id.clone(),
        owner.root_agent.clone(),
        spec(),
        10,
    );
    planned.state = loopal_protocol::WorkflowRunState::Cancelled;
    planned.revision = 1;
    let journal = loopal_storage::WorkflowJournal::from_session_store(
        sessions.as_ref(),
        &owner.session_id,
        run_id.clone(),
    )
    .unwrap();
    let mut init = planned.clone();
    init.state = loopal_protocol::WorkflowRunState::Planned;
    init.revision = 0;
    journal.append_init(init).unwrap();
    storage
        .append_commit(
            &owner,
            &run_id,
            vec![loopal_protocol::WorkflowEvent {
                run_id: run_id.clone(),
                revision: 1,
                occurred_at_unix_ms: 11,
                payload: loopal_protocol::WorkflowEventPayload::CancelRequested { reason: None },
            }],
            None,
        )
        .unwrap();
    let delivery_id = WorkflowTerminalDeliveryId::new(&owner.session_id, run_id.clone(), 1);
    let intent = WorkflowTerminalNotification {
        delivery_id: delivery_id.clone(),
        state: loopal_protocol::WorkflowRunState::Cancelled,
        run_goal: planned.spec.run_goal,
        outcome: loopal_protocol::WorkflowTerminalOutcome::Cancelled {
            reason: "cancelled".into(),
        },
        content: "cancelled".into(),
    };
    assert!(matches!(
        storage.append_delivery_intent(&owner, intent.clone()).unwrap(),
        WorkflowJournalDeliveryIntentOutcome::Appended(actual) if actual == intent
    ));
    assert!(matches!(
        storage.append_delivery_intent(&owner, intent.clone()).unwrap(),
        WorkflowJournalDeliveryIntentOutcome::AlreadyPresent(actual) if actual == intent
    ));
    assert_eq!(
        storage.append_delivery_ack(&owner, &delivery_id).unwrap(),
        WorkflowJournalDeliveryAckOutcome::Appended
    );
    assert_eq!(
        storage.append_delivery_ack(&owner, &delivery_id).unwrap(),
        WorkflowJournalDeliveryAckOutcome::AlreadyPresent
    );

    let wrong_session = WorkflowTerminalDeliveryId::new("other-session", run_id, 1);
    assert_eq!(
        storage.append_delivery_ack(&owner, &wrong_session),
        Err(super::super::WorkflowCoordinatorError::RecoveryInvalid)
    );
    let mut wrong_intent = intent;
    wrong_intent.delivery_id = wrong_session;
    assert_eq!(
        storage.append_delivery_intent(&owner, wrong_intent),
        Err(super::super::WorkflowCoordinatorError::RecoveryInvalid)
    );
}

#[test]
fn production_adapter_redacts_before_durable_append() {
    let temp = tempfile::tempdir().unwrap();
    let sessions = Arc::new(SessionStore::with_base_dir(temp.path().to_path_buf()));
    let seed = FinalSinkRedactionSeed::new();
    seed.observe("token", "journal-secret".into()).unwrap();
    let storage = SessionWorkflowJournals::new_with_redaction_seed(sessions.clone(), seed);
    let owner = owner("session-redaction", "root");
    let run_id = WorkflowRunId::new("wrun_redaction");
    let mut start = request("wreq_redaction");
    start.spec.run_goal = "journal-secret".into();
    let planned = loopal_protocol::WorkflowRunSnapshot::planned(
        run_id.clone(),
        owner.root_agent.clone(),
        start.spec.clone(),
        10,
    );
    let event = loopal_protocol::WorkflowEvent {
        run_id: run_id.clone(),
        revision: 1,
        occurred_at_unix_ms: 11,
        payload: loopal_protocol::WorkflowEventPayload::CancelRequested {
            reason: Some("journal-secret".into()),
        },
    };
    let snapshot = apply_event(&planned, &event).unwrap();
    let record = loopal_protocol::WorkflowRequestRecord {
        request_id: start.request_id.clone(),
        operation: "start".into(),
        payload: serde_json::to_value(start).unwrap(),
        response: serde_json::to_value(loopal_protocol::WorkflowStartResponse {
            summary: loopal_protocol::WorkflowRunSummary::from(&snapshot),
        })
        .unwrap(),
    };
    storage
        .append_start(StartJournalRecord {
            owner: owner.clone(),
            planned,
            event,
            request: record,
        })
        .unwrap();
    let path = sessions
        .workflow_journal_path(&owner.session_id, run_id.as_str())
        .unwrap();
    let written = std::fs::read_to_string(path).unwrap();
    assert!(written.contains("<secret_ref:token>"));
    assert!(!written.contains("journal-secret"));
}
