#![cfg(windows)]

use std::io::Write;

use loopal_storage::{SessionStore, WorkflowJournalError};

#[path = "suite/workflow_journal_support.rs"]
mod workflow_journal_support;
use workflow_journal_support::*;

fn assert_corruption<T>(result: Result<T, WorkflowJournalError>) {
    assert!(matches!(
        result,
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

fn junction(target: &std::path::Path, link: &std::path::Path) {
    let script = r#"
$ErrorActionPreference = 'Stop'
New-Item -ItemType Junction -Path $env:LOOPAL_JUNCTION_LINK -Target $env:LOOPAL_JUNCTION_TARGET -ErrorAction Stop | Out-Null
"#;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("LOOPAL_JUNCTION_LINK", link)
        .env("LOOPAL_JUNCTION_TARGET", target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "junction creation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn leaf_reparse_point_is_rejected_without_touching_target() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("outside");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("sentinel"), b"outside sentinel").unwrap();
    let journal_path = path(&temp);
    std::fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
    junction(&target, &journal_path);

    assert!(journal(&temp).replay().is_err());
    assert!(journal(&temp).append_init(snapshot()).is_err());
    assert_eq!(
        std::fs::read(target.join("sentinel")).unwrap(),
        b"outside sentinel"
    );
}

#[test]
fn parent_reparse_point_is_rejected_without_escape() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let session = temp.path().join("sessions/session-one");
    std::fs::create_dir_all(&session).unwrap();
    junction(outside.path(), &session.join("workflows"));

    let store = SessionStore::with_base_dir(temp.path().to_path_buf());
    assert_corruption(store.list_workflow_run_ids("session-one"));
    assert!(journal(&temp).append_init(snapshot()).is_err());
    assert!(!outside.path().join("wrun_test.jsonl").exists());
}

#[test]
fn multiply_linked_journal_is_rejected_for_read_and_append() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    let before = std::fs::read(path(&temp)).unwrap();
    std::fs::hard_link(path(&temp), temp.path().join("journal-alias.jsonl")).unwrap();

    assert_corruption(journal.replay());
    assert!(journal.append_commit(Vec::new(), Some(request())).is_err());
    assert_eq!(std::fs::read(path(&temp)).unwrap(), before);
}

#[test]
fn discovered_identity_rejects_replacement() {
    let temp = tempfile::tempdir().unwrap();
    journal(&temp).append_init(snapshot()).unwrap();
    let store = SessionStore::with_base_dir(temp.path().to_path_buf());
    let mut discovered = store.list_workflow_journals("session-one").unwrap();
    let replacement = temp.path().join("replacement.jsonl");
    std::fs::write(&replacement, std::fs::read(path(&temp)).unwrap()).unwrap();
    std::fs::remove_file(path(&temp)).unwrap();
    std::fs::rename(replacement, path(&temp)).unwrap();

    let journal = discovered.remove(0);
    assert_corruption(journal.replay());
    assert!(journal.append_commit(Vec::new(), Some(request())).is_err());
}

#[test]
fn torn_tail_is_repaired_on_the_same_file_identity() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    let good = std::fs::metadata(path(&temp)).unwrap().len();
    std::fs::OpenOptions::new()
        .append(true)
        .open(path(&temp))
        .unwrap()
        .write_all(b"torn")
        .unwrap();

    let tail = journal.replay().unwrap().torn_tail.unwrap();
    journal.repair_torn_tail(tail).unwrap();
    assert_eq!(std::fs::metadata(path(&temp)).unwrap().len(), good);
    assert!(journal.replay().unwrap().torn_tail.is_none());
}
