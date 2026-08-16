use std::io::Write;

use loopal_storage::{SessionStore, WorkflowJournalError};

use crate::workflow_journal_support::*;

fn replace_file(replacement: &std::path::Path, destination: &std::path::Path) {
    #[cfg(windows)]
    std::fs::remove_file(destination).unwrap();
    std::fs::rename(replacement, destination).unwrap();
}

#[cfg(unix)]
#[test]
fn journal_leaf_symlink_cannot_be_read_or_written() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("outside.jsonl");
    std::fs::write(&target, b"outside sentinel").unwrap();
    let journal_path = path(&temp);
    std::fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &journal_path).unwrap();

    assert!(matches!(
        journal(&temp).replay(),
        Err(WorkflowJournalError::Corruption { .. })
    ));
    assert!(journal(&temp).append_init(snapshot()).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"outside sentinel");
}

#[cfg(unix)]
#[test]
fn parent_directory_symlink_cannot_escape_session_store() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let session = temp.path().join("sessions/session-one");
    std::fs::create_dir_all(&session).unwrap();
    std::os::unix::fs::symlink(outside.path(), session.join("workflows")).unwrap();

    assert!(matches!(
        SessionStore::with_base_dir(temp.path().to_path_buf()).list_workflow_run_ids("session-one"),
        Err(WorkflowJournalError::Corruption { .. })
    ));
    assert!(journal(&temp).append_init(snapshot()).is_err());
    assert!(!outside.path().join("wrun_test.jsonl").exists());
}

#[test]
fn replacement_after_discovery_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    let store = SessionStore::with_base_dir(temp.path().to_path_buf());
    let mut discovered = store.list_workflow_journals("session-one").unwrap();
    assert_eq!(discovered.len(), 1);

    let replacement = temp.path().join("replacement.jsonl");
    std::fs::write(&replacement, std::fs::read(path(&temp)).unwrap()).unwrap();
    replace_file(&replacement, &path(&temp));

    assert!(matches!(
        discovered.remove(0).replay(),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

#[test]
fn torn_tail_repair_rejects_same_length_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(path(&temp))
        .unwrap()
        .write_all(b"torn")
        .unwrap();
    let tail = journal.replay().unwrap().torn_tail.unwrap();
    let replacement = temp.path().join("replacement.jsonl");
    let original = std::fs::read(path(&temp)).unwrap();
    std::fs::write(&replacement, vec![b'x'; original.len()]).unwrap();
    replace_file(&replacement, &path(&temp));

    assert!(matches!(
        journal.repair_torn_tail(tail),
        Err(WorkflowJournalError::RepairMismatch { .. })
    ));
    assert_eq!(
        std::fs::read(path(&temp)).unwrap(),
        vec![b'x'; original.len()]
    );
}
