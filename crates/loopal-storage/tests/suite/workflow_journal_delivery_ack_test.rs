use std::io::Write;

use loopal_protocol::{
    WorkflowRunState, WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
    WorkflowTerminalOutcome,
};
use loopal_storage::{
    WorkflowJournalAppendDecision, WorkflowJournalAppendKind, WorkflowJournalError,
};

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

fn terminal_journal(temp: &tempfile::TempDir) -> loopal_storage::WorkflowJournal {
    let journal = journal(temp);
    journal.append_init(snapshot()).unwrap();
    journal.append_commit(vec![event(1)], None).unwrap();
    journal.append_commit(vec![cancel_event(2)], None).unwrap();
    journal
}

fn cancel_event(revision: u64) -> loopal_protocol::WorkflowEvent {
    loopal_protocol::WorkflowEvent {
        run_id: "wrun_test".into(),
        revision,
        occurred_at_unix_ms: 100 + revision,
        payload: loopal_protocol::WorkflowEventPayload::CancelRequested { reason: None },
    }
}

#[test]
fn delivery_ack_round_trips_without_advancing_revision() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    let id = delivery(2);
    let intent = notification(2);

    assert_eq!(
        journal.append_delivery_intent(intent.clone()).unwrap(),
        WorkflowJournalAppendDecision::Append
    );
    assert_eq!(
        journal.append_delivery_ack(id.clone()).unwrap(),
        WorkflowJournalAppendDecision::Append
    );
    let replay = journal.replay().unwrap();
    assert_eq!(replay.delivery_intents, vec![intent]);
    assert_eq!(replay.delivery_acks, vec![id]);
    assert_eq!(replay.entry_count(), 5);
    assert!(
        std::fs::read_to_string(path(&temp))
            .unwrap()
            .contains(r#""kind":"delivery_intent""#)
    );
    assert!(
        std::fs::read_to_string(path(&temp))
            .unwrap()
            .contains(r#""kind":"delivery_ack""#)
    );
    assert!(journal.append_commit(vec![cancel_event(3)], None).is_err());
}

#[test]
fn duplicate_ack_is_idempotent_and_does_not_grow_journal() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    let id = delivery(2);
    journal.append_delivery_intent(notification(2)).unwrap();
    journal.append_delivery_ack(id.clone()).unwrap();
    let before = std::fs::metadata(path(&temp)).unwrap().len();

    assert_eq!(
        journal.append_delivery_ack(id.clone()).unwrap(),
        WorkflowJournalAppendDecision::AlreadyPresent
    );
    assert_eq!(
        journal
            .preflight_append(WorkflowJournalAppendKind::DeliveryAck(id))
            .unwrap(),
        WorkflowJournalAppendDecision::AlreadyPresent
    );
    assert_eq!(std::fs::metadata(path(&temp)).unwrap().len(), before);
}

#[test]
fn duplicate_intent_is_idempotent_and_conflicting_payload_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    let intent = notification(2);
    assert_eq!(
        journal.append_delivery_intent(intent.clone()).unwrap(),
        WorkflowJournalAppendDecision::Append
    );
    let before = std::fs::metadata(path(&temp)).unwrap().len();
    assert_eq!(
        journal.append_delivery_intent(intent.clone()).unwrap(),
        WorkflowJournalAppendDecision::AlreadyPresent
    );
    assert_eq!(std::fs::metadata(path(&temp)).unwrap().len(), before);

    let mut conflict = intent;
    conflict.content.push_str(" changed");
    assert!(journal.append_delivery_intent(conflict).is_err());
    assert_eq!(std::fs::metadata(path(&temp)).unwrap().len(), before);
}

#[test]
fn wrong_identity_revision_and_unknown_fields_fail_closed() {
    let cases = [
        WorkflowTerminalDeliveryId::new("session-other", "wrun_test".into(), 2),
        WorkflowTerminalDeliveryId::new("session-one", "wrun_other".into(), 2),
        delivery(0),
        delivery(1),
    ];
    for id in cases {
        let temp = tempfile::tempdir().unwrap();
        assert!(terminal_journal(&temp).append_delivery_ack(id).is_err());
    }

    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    let line = concat!(
        r#"{"kind":"delivery_ack","version":1,"delivery_id":{"session_id":"session-one","run_id":"wrun_test","terminal_revision":2,"extra":true}}"#,
        "\n"
    );
    std::fs::OpenOptions::new()
        .append(true)
        .open(path(&temp))
        .unwrap()
        .write_all(line.as_bytes())
        .unwrap();
    assert!(matches!(
        journal.replay(),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

#[test]
fn torn_ack_tail_is_reported_and_repairable() {
    let temp = tempfile::tempdir().unwrap();
    let journal = terminal_journal(&temp);
    let good = std::fs::metadata(path(&temp)).unwrap().len();
    std::fs::OpenOptions::new()
        .append(true)
        .open(path(&temp))
        .unwrap()
        .write_all(br#"{"kind":"delivery_ack""#)
        .unwrap();
    let tail = journal.replay().unwrap().torn_tail.unwrap();
    assert_eq!(tail.good_offset(), good);
    journal.repair_torn_tail(tail).unwrap();
}

#[test]
fn nonterminal_delivery_intent_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    let mut invalid = notification(0);
    invalid.delivery_id.terminal_revision = 0;
    assert!(journal.append_delivery_intent(invalid).is_err());
}
