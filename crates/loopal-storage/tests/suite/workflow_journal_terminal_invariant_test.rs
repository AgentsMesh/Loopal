use std::io::Write;

use loopal_protocol::{
    WorkflowRunState, WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
    WorkflowTerminalOutcome,
};
use loopal_storage::{WorkflowJournalAppendDecision, WorkflowJournalError};

use crate::workflow_journal_support::*;

fn delivery(revision: u64) -> WorkflowTerminalDeliveryId {
    WorkflowTerminalDeliveryId::new("session-one", "wrun_test".into(), revision)
}

fn notification(revision: u64) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: delivery(revision),
        state: WorkflowRunState::Cancelled,
        run_goal: "goal <secret_ref:token>".into(),
        outcome: WorkflowTerminalOutcome::Cancelled {
            reason: "Workflow was cancelled before completion.".into(),
        },
        content: "Workflow wrun_test was cancelled.".into(),
    }
}

fn cancel_event(revision: u64) -> loopal_protocol::WorkflowEvent {
    loopal_protocol::WorkflowEvent {
        run_id: "wrun_test".into(),
        revision,
        occurred_at_unix_ms: 100 + revision,
        payload: loopal_protocol::WorkflowEventPayload::CancelRequested { reason: None },
    }
}

fn terminal_journal(temp: &tempfile::TempDir) -> loopal_storage::WorkflowJournal {
    let journal = journal(temp);
    journal.append_init(snapshot()).unwrap();
    journal.append_commit(vec![event(1)], None).unwrap();
    journal.append_commit(vec![cancel_event(2)], None).unwrap();
    journal
}

#[test]
fn delivery_intent_requires_the_current_snapshot_to_be_terminal() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    journal.append_commit(vec![event(1)], None).unwrap();

    assert!(journal.append_delivery_intent(notification(1)).is_err());
}

#[test]
fn delivery_intent_state_must_match_the_current_terminal_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    let mut intent = notification(2);
    intent.state = WorkflowRunState::Failed;
    intent.outcome = WorkflowTerminalOutcome::Failed {
        class: loopal_protocol::WorkflowFailureClass::Permanent,
        reason: "failed".into(),
    };

    assert!(journal.append_delivery_intent(intent).is_err());
}

#[test]
fn no_commit_kind_can_follow_delivery_intent() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    journal.append_delivery_intent(notification(2)).unwrap();
    let before = std::fs::metadata(path(&temp)).unwrap().len();

    assert!(journal.append_commit(Vec::new(), Some(request())).is_err());
    assert_eq!(std::fs::metadata(path(&temp)).unwrap().len(), before);
}

#[test]
fn acknowledgement_requires_prior_intent_and_exact_identity() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    assert!(journal.append_delivery_ack(delivery(2)).is_err());
    journal.append_delivery_intent(notification(2)).unwrap();

    for wrong in [
        WorkflowTerminalDeliveryId::new("session-other", "wrun_test".into(), 2),
        WorkflowTerminalDeliveryId::new("session-one", "wrun_other".into(), 2),
        delivery(1),
    ] {
        assert!(journal.append_delivery_ack(wrong).is_err());
    }
    assert_eq!(
        journal.append_delivery_ack(delivery(2)).unwrap(),
        WorkflowJournalAppendDecision::Append
    );
}

#[test]
fn replay_rejects_a_commit_persisted_after_delivery_intent() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    journal.append_delivery_intent(notification(2)).unwrap();
    let commit = serde_json::json!({
        "kind": "commit",
        "version": 1,
        "run_id": "wrun_test",
        "events": [],
        "request": request(),
    });
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .open(path(&temp))
            .unwrap(),
        "{commit}"
    )
    .unwrap();

    assert!(matches!(
        journal.replay(),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}
