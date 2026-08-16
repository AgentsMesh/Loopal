use super::{OpenMode, open, workflows_directory};
use crate::workflow_journal::fs::JournalLocation;

#[test]
fn open_parent_handles_block_directory_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let (guards, directory) = workflows_directory(temp.path(), "session-one", true)
        .unwrap_or_else(|_| panic!("workflow directory creation failed"));
    let replacement = temp.path().join("replacement");
    let moved = temp.path().join("workflows-moved");
    std::fs::create_dir(&replacement).unwrap();

    assert!(std::fs::rename(&directory, &moved).is_err());
    drop(guards);
    std::fs::rename(&directory, &moved).unwrap();
    std::fs::rename(&replacement, &directory).unwrap();
}

#[test]
fn append_handle_is_exclusive_until_released() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp
        .path()
        .join("sessions/session-one/workflows/wrun_test.jsonl");
    let location = JournalLocation::new(temp.path(), "session-one", "wrun_test", path);
    let first = open(&location, OpenMode::AppendCreate)
        .unwrap_or_else(|_| panic!("initial journal open failed"));

    assert!(open(&location, OpenMode::AppendExisting).is_err());
    drop(first);
    let reopened = open(&location, OpenMode::AppendExisting)
        .unwrap_or_else(|_| panic!("journal did not reopen after writer release"));
    drop(reopened);
}
