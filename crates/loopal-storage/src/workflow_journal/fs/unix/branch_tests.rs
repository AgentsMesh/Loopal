use std::ffi::OsStr;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;

use super::{OpenMode, lock, metadata, open_directory_at, validate_regular};

#[test]
fn directory_creation_reports_permission_failure() {
    let temp = tempfile::tempdir().unwrap();
    let parent = File::open(temp.path()).unwrap();
    std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o500)).unwrap();

    let result = open_directory_at(&parent, OsStr::new("missing"), true);

    std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
}

#[test]
fn closed_descriptors_report_lock_and_metadata_errors() {
    let temp = tempfile::tempdir().unwrap();

    let lock_file = File::open(temp.path()).unwrap();
    assert_eq!(unsafe { libc::close(lock_file.as_raw_fd()) }, 0);
    assert!(lock(&lock_file, OpenMode::Read).is_err());
    std::mem::forget(lock_file);

    let metadata_file = File::open(temp.path()).unwrap();
    assert_eq!(unsafe { libc::close(metadata_file.as_raw_fd()) }, 0);
    assert!(metadata(&metadata_file).is_err());
    std::mem::forget(metadata_file);
}

#[test]
fn hardlinked_journal_fails_private_regular_validation() {
    let temp = tempfile::tempdir().unwrap();
    let journal = temp.path().join("journal.jsonl");
    let alias = temp.path().join("alias.jsonl");
    std::fs::write(&journal, b"record\n").unwrap();
    std::fs::hard_link(&journal, alias).unwrap();
    let file = File::open(journal).unwrap();
    let metadata = match metadata(&file) {
        Ok(metadata) => metadata,
        Err(_) => panic!("regular file metadata failed"),
    };

    assert!(validate_regular(&metadata).is_err());
}
