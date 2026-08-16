use std::io::Write;

use loopal_storage::{
    MAX_WORKFLOW_JOURNAL_ENTRIES, WorkflowJournalAppendDecision, WorkflowJournalAppendKind,
    WorkflowJournalError, WorkflowJournalLimit,
};

use crate::workflow_journal_support::*;

#[test]
fn preflight_reports_init_state_and_requires_init_for_commit() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    assert_eq!(
        journal
            .preflight_append(WorkflowJournalAppendKind::Init)
            .unwrap(),
        WorkflowJournalAppendDecision::Append
    );
    assert!(matches!(
        journal.preflight_append(WorkflowJournalAppendKind::Commit),
        Err(WorkflowJournalError::Corruption { .. })
    ));

    journal.append_init(snapshot()).unwrap();
    assert_eq!(
        journal
            .preflight_append(WorkflowJournalAppendKind::Init)
            .unwrap(),
        WorkflowJournalAppendDecision::AlreadyPresent
    );
    assert_eq!(
        journal
            .preflight_append(WorkflowJournalAppendKind::Commit)
            .unwrap(),
        WorkflowJournalAppendDecision::Append
    );
}

#[test]
fn preflight_rejects_torn_tail_before_append() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path(&temp))
        .unwrap()
        .write_all(b"{")
        .unwrap();
    assert!(matches!(
        journal.preflight_append(WorkflowJournalAppendKind::Commit),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

#[test]
fn preflight_rejects_journal_at_entry_limit() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    write_init(&path);
    let line = concat!(
        r#"{"kind":"commit","version":1,"run_id":"wrun_test","events":[],"request":{"request_id":"wreq_get","operation":"get","payload":{"request_id":"wreq_get","run_id":"wrun_test"},"response":{"run":null}}}"#,
        "\n"
    );
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    for _ in 1..MAX_WORKFLOW_JOURNAL_ENTRIES {
        file.write_all(line.as_bytes()).unwrap();
    }

    assert!(matches!(
        journal(&temp).preflight_append(WorkflowJournalAppendKind::Commit),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::Entries,
            ..
        })
    ));
}
