use std::io::Write;

use loopal_storage::{WorkflowJournalError, WorkflowJournalLimit};

use crate::workflow_journal_support::*;

#[test]
fn missing_journal_replays_empty() {
    let temp = tempfile::tempdir().unwrap();
    let replay = journal(&temp).replay().unwrap();
    assert!(replay.init.is_none());
    assert!(replay.commits.is_empty());
    assert!(replay.torn_tail.is_none());
    assert_eq!(replay.last_good_offset, 0);
}

#[test]
fn final_torn_tail_reports_offset_and_repairs() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    let good = std::fs::metadata(path(&temp)).unwrap().len();
    std::fs::OpenOptions::new()
        .append(true)
        .open(path(&temp))
        .unwrap()
        .write_all(br#"{"kind":"commit""#)
        .unwrap();

    let replay = journal.replay().unwrap();
    let tail = replay.torn_tail.unwrap();
    assert_eq!(tail.good_offset(), good);
    journal.repair_torn_tail(tail).unwrap();
    assert_eq!(std::fs::metadata(path(&temp)).unwrap().len(), good);
    assert!(journal.replay().unwrap().torn_tail.is_none());
}

#[test]
fn middle_and_final_newline_corruption_hard_fail() {
    for bytes in [b"not json\n".as_slice(), b"not json\n{}\n".as_slice()] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        write_init(&path);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        assert!(matches!(
            journal(&temp).replay(),
            Err(WorkflowJournalError::Corruption { .. })
        ));
    }
}

#[test]
fn legacy_init_without_events_replays_with_empty_events() {
    let temp = tempfile::tempdir().unwrap();
    write_init(&path(&temp));
    let init = journal(&temp).replay().unwrap().init.unwrap();
    assert!(init.events.is_empty());
}

#[test]
fn unknown_field_and_version_are_corruption() {
    for mutation in [
        ("unknown", serde_json::json!(true)),
        ("version", serde_json::json!(2)),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        let mut value = serde_json::json!({
            "kind": "init",
            "version": 1,
            "snapshot": snapshot(),
        });
        value[mutation.0] = mutation.1;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{}\n", value)).unwrap();
        assert!(matches!(
            journal(&temp).replay(),
            Err(WorkflowJournalError::Corruption { .. })
        ));
    }
}

#[test]
fn append_rejects_commit_before_init_and_second_init() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    assert!(matches!(
        journal.append_commit(vec![event(1)], None),
        Err(WorkflowJournalError::Corruption { .. })
    ));

    journal.append_init(snapshot()).unwrap();
    assert!(matches!(
        journal.append_init(snapshot()),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

#[test]
fn append_rejects_cross_commit_revision_gap_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    journal.append_commit(vec![event(1)], None).unwrap();
    journal.append_commit(Vec::new(), Some(request())).unwrap();
    let before = std::fs::metadata(path(&temp)).unwrap().len();

    assert!(matches!(
        journal.append_commit(vec![event(3)], None),
        Err(WorkflowJournalError::Corruption { .. })
    ));
    assert_eq!(std::fs::metadata(path(&temp)).unwrap().len(), before);

    journal.append_commit(vec![event(2)], None).unwrap();
    let replay = journal.replay().unwrap();
    assert_eq!(replay.commits.last().unwrap().events[0].revision, 2);
}

#[test]
fn oversized_line_is_bounded_corruption() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(loopal_storage::MAX_WORKFLOW_JOURNAL_LINE_BYTES as u64 + 1)
        .unwrap();
    assert!(matches!(
        journal(&temp).replay(),
        Err(WorkflowJournalError::Corruption { .. })
            | Err(WorkflowJournalError::LimitExceeded {
                limit: WorkflowJournalLimit::LineBytes,
                ..
            })
    ));
}
