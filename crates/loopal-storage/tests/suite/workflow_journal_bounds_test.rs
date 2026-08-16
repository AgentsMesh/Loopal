use std::io::{Seek, SeekFrom, Write};

use loopal_storage::{
    MAX_WORKFLOW_JOURNAL_ENTRIES, MAX_WORKFLOW_JOURNAL_TOTAL_BYTES, WorkflowJournalError,
    WorkflowJournalLimit,
};

use crate::workflow_journal_support::*;

#[test]
fn replay_rejects_entry_count_over_limit() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    write_init(&path);
    let commit = serde_json::json!({
        "kind": "commit",
        "version": 1,
        "run_id": "wrun_test",
        "events": [],
        "request": request(),
    });
    let line = format!("{}\n", commit);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    for _ in 1..=MAX_WORKFLOW_JOURNAL_ENTRIES {
        file.write_all(line.as_bytes()).unwrap();
    }
    assert!(matches!(
        journal(&temp).replay(),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::Entries,
            ..
        })
    ));
}

#[test]
fn replay_rejects_total_bytes_before_reading_lines() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = std::fs::File::create(&path).unwrap();
    file.seek(SeekFrom::Start(MAX_WORKFLOW_JOURNAL_TOTAL_BYTES))
        .unwrap();
    file.write_all(b"x").unwrap();
    assert!(matches!(
        journal(&temp).replay(),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::TotalBytes,
            ..
        })
    ));
}
