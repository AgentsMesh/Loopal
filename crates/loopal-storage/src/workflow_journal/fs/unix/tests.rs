use std::ffi::OsStr;
use std::io::Write;

use super::{OpenMode, open_journal_at, workflows_directory};

#[test]
fn pinned_workflows_directory_cannot_redirect_create() {
    let temp = tempfile::tempdir().unwrap();
    let directory = workflows_directory(temp.path(), "session-one", true)
        .unwrap_or_else(|_| panic!("workflow directory creation failed"));
    let session = temp.path().join("sessions/session-one");
    let canonical = session.join("workflows");
    let pinned = session.join("workflows-pinned");
    std::fs::rename(&canonical, &pinned).unwrap();
    std::fs::create_dir(&canonical).unwrap();

    let mut opened = open_journal_at(
        &directory,
        OsStr::new("wrun_test.jsonl"),
        OpenMode::AppendCreate,
    )
    .unwrap_or_else(|_| panic!("descriptor-relative journal creation failed"));
    opened.file.write_all(b"pinned\n").unwrap();
    opened.file.sync_data().unwrap();

    assert_eq!(
        std::fs::read(pinned.join("wrun_test.jsonl")).unwrap(),
        b"pinned\n"
    );
    assert!(!canonical.join("wrun_test.jsonl").exists());
}
