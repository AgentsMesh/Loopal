use std::path::Path;

use loopal_protocol::WorkflowRunId;

use super::apply;
use super::test_support::{assert_corrupt, delivery_id, init_entry, notification, terminal_replay};
use crate::workflow_journal::record::WorkflowJournalEntry;
use crate::workflow_journal::replay::WorkflowJournalReplay;

#[test]
fn apply_accepts_each_ordered_entry_kind() {
    let path = Path::new("journal.jsonl");
    let mut replay = WorkflowJournalReplay::default();
    apply(&mut replay, init_entry(), path, 0).unwrap();
    apply(
        &mut replay,
        WorkflowJournalEntry::commit(WorkflowRunId::new("wrun_read_test"), Vec::new(), None),
        path,
        1,
    )
    .unwrap();
    assert_eq!(replay.entry_count(), 2);

    let mut replay = terminal_replay();
    apply(
        &mut replay,
        WorkflowJournalEntry::delivery_intent(notification()),
        path,
        2,
    )
    .unwrap();
    apply(
        &mut replay,
        WorkflowJournalEntry::delivery_ack(delivery_id()),
        path,
        3,
    )
    .unwrap();
    assert_eq!(replay.delivery_intents, vec![notification()]);
    assert_eq!(replay.delivery_acks, vec![delivery_id()]);
}

#[test]
fn apply_rejects_entries_before_init_and_duplicate_init() {
    let path = Path::new("journal.jsonl");
    let mut replay = WorkflowJournalReplay::default();
    assert_corrupt(
        apply(
            &mut replay,
            WorkflowJournalEntry::commit(WorkflowRunId::new("wrun_read_test"), Vec::new(), None),
            path,
            1,
        ),
        "commit encountered before init",
    );
    assert_corrupt(
        apply(
            &mut replay,
            WorkflowJournalEntry::delivery_intent(notification()),
            path,
            2,
        ),
        "delivery intent encountered before init",
    );
    assert_corrupt(
        apply(
            &mut replay,
            WorkflowJournalEntry::delivery_ack(delivery_id()),
            path,
            3,
        ),
        "delivery acknowledgement encountered before init",
    );

    apply(&mut replay, init_entry(), path, 4).unwrap();
    assert_corrupt(
        apply(&mut replay, init_entry(), path, 5),
        "init must be the first and only init entry",
    );
}

#[test]
fn apply_rejects_duplicate_late_and_unmatched_delivery_records() {
    let path = Path::new("journal.jsonl");
    let mut replay = terminal_replay();
    assert_corrupt(
        apply(
            &mut replay,
            WorkflowJournalEntry::delivery_ack(delivery_id()),
            path,
            6,
        ),
        "lacks one matching unacknowledged intent",
    );
    apply(
        &mut replay,
        WorkflowJournalEntry::delivery_intent(notification()),
        path,
        7,
    )
    .unwrap();
    assert_corrupt(
        apply(
            &mut replay,
            WorkflowJournalEntry::delivery_intent(notification()),
            path,
            8,
        ),
        "duplicate or late delivery intent",
    );
    assert_corrupt(
        apply(
            &mut replay,
            WorkflowJournalEntry::commit(WorkflowRunId::new("wrun_read_test"), Vec::new(), None),
            path,
            9,
        ),
        "commit encountered before init",
    );
    apply(
        &mut replay,
        WorkflowJournalEntry::delivery_ack(delivery_id()),
        path,
        10,
    )
    .unwrap();
    assert_corrupt(
        apply(
            &mut replay,
            WorkflowJournalEntry::delivery_ack(delivery_id()),
            path,
            11,
        ),
        "lacks one matching unacknowledged intent",
    );

    let mut ack_only = terminal_replay();
    ack_only.delivery_acks.push(delivery_id());
    assert_corrupt(
        apply(
            &mut ack_only,
            WorkflowJournalEntry::delivery_intent(notification()),
            path,
            12,
        ),
        "duplicate or late delivery intent",
    );
    assert_corrupt(
        apply(
            &mut ack_only,
            WorkflowJournalEntry::commit(WorkflowRunId::new("wrun_read_test"), Vec::new(), None),
            path,
            13,
        ),
        "commit encountered before init",
    );
}
