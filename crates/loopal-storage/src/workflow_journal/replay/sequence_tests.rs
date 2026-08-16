use std::path::Path;

use loopal_protocol::{
    WorkflowEvent, WorkflowEventPayload, WorkflowRunId, WorkflowRunState,
    WorkflowTerminalDeliveryId, WorkflowTerminalNotification, WorkflowTerminalOutcome,
};

use super::{WorkflowJournalEntry, validate};
use crate::workflow_journal::error::WorkflowJournalError;
use crate::workflow_journal::record::WORKFLOW_JOURNAL_VERSION;

fn event(run_id: &WorkflowRunId, revision: u64) -> WorkflowEvent {
    WorkflowEvent {
        run_id: run_id.clone(),
        revision,
        occurred_at_unix_ms: 1,
        payload: WorkflowEventPayload::CancelRequested { reason: None },
    }
}

#[test]
fn contiguous_commit_advances_revision() {
    let run_id = WorkflowRunId::from("wrun_sequence");
    let entry = WorkflowJournalEntry::Commit {
        version: WORKFLOW_JOURNAL_VERSION,
        run_id: run_id.clone(),
        events: vec![event(&run_id, 1), event(&run_id, 2)],
        request: None,
    };
    let mut revision = 0;

    validate(&entry, &mut revision, Path::new("journal"), 9).unwrap();
    assert_eq!(revision, 2);
}

#[test]
fn revision_gap_reports_path_offset_and_expected_revision() {
    let run_id = WorkflowRunId::from("wrun_sequence");
    let entry = WorkflowJournalEntry::commit(run_id.clone(), vec![event(&run_id, 2)], None);
    let mut revision = 0;
    let path = Path::new("runs/journal.jsonl");

    let error = validate(&entry, &mut revision, path, 41).unwrap_err();
    match error {
        WorkflowJournalError::Corruption {
            path: actual,
            offset,
            detail,
        } => {
            assert_eq!(actual, path);
            assert_eq!(offset, 41);
            assert!(detail.contains("expected 1, found 2"));
        }
        other => panic!("unexpected error: {other}"),
    }
    assert_eq!(revision, 0);
}

fn delivery_id(revision: u64) -> WorkflowTerminalDeliveryId {
    WorkflowTerminalDeliveryId::new("session-sequence", "wrun_sequence".into(), revision)
}

#[test]
fn delivery_records_require_the_exact_terminal_revision() {
    let path = Path::new("runs/journal.jsonl");
    let notification = WorkflowTerminalNotification {
        delivery_id: delivery_id(2),
        state: WorkflowRunState::Cancelled,
        run_goal: "sequence".into(),
        outcome: WorkflowTerminalOutcome::Cancelled {
            reason: "cancelled".into(),
        },
        content: "Workflow cancelled.".into(),
    };
    let mut revision = 2;
    validate(
        &WorkflowJournalEntry::delivery_intent(notification),
        &mut revision,
        path,
        51,
    )
    .unwrap();
    validate(
        &WorkflowJournalEntry::delivery_ack(delivery_id(2)),
        &mut revision,
        path,
        52,
    )
    .unwrap();
    assert_eq!(revision, 2);

    let error = validate(
        &WorkflowJournalEntry::delivery_ack(delivery_id(3)),
        &mut revision,
        path,
        53,
    )
    .unwrap_err();
    match error {
        WorkflowJournalError::Corruption {
            path: actual,
            offset,
            detail,
        } => {
            assert_eq!(actual, path);
            assert_eq!(offset, 53);
            assert!(detail.contains("expected 2, found 3"));
        }
        other => panic!("unexpected error: {other}"),
    }
}
