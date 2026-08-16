use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "loopal-overflow-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

struct FailingWriter {
    fail_flush: bool,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_flush {
            Ok(bytes.len())
        } else {
            Err(io::Error::other("write failed"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::other("flush failed"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn write_and_flush_failures_remove_partial_file() {
    for fail_flush in [false, true] {
        let root = fixture();
        let path = root.join("partial.txt");
        std::fs::write(&path, "partial").unwrap();
        let error = write_file(&path, FailingWriter { fail_flush }, b"guarded").unwrap_err();

        if fail_flush {
            assert!(matches!(error, OverflowPersistenceError::Flush(_)));
        } else {
            assert!(matches!(error, OverflowPersistenceError::Write(_)));
        }
        assert!(!path.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn create_file_rejects_missing_directory() {
    let root = fixture();
    let missing = root.join("missing");

    let error = create_file_with(&missing, "output", || 1, |_| Ok(())).unwrap_err();

    assert!(matches!(error, OverflowPersistenceError::Open(_)));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn create_file_removes_file_when_permission_hardening_fails() {
    let root = fixture();
    let expected = root.join(format!("output_{}_7.txt", std::process::id()));

    let error = create_file_with(
        &root,
        "output",
        || 7,
        |_| Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
    )
    .unwrap_err();

    assert!(matches!(error, OverflowPersistenceError::Open(_)));
    assert!(!expected.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn create_file_exhausts_collision_retries() {
    let root = fixture();
    let existing = root.join(format!("output_{}_9.txt", std::process::id()));
    std::fs::write(&existing, "occupied").unwrap();

    let error = create_file_with(&root, "output", || 9, |_| Ok(())).unwrap_err();

    assert!(matches!(error, OverflowPersistenceError::Open(_)));
    assert_eq!(std::fs::read_to_string(existing).unwrap(), "occupied");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn labels_are_bounded_and_never_empty() {
    assert_eq!(safe_label(""), "output");
    assert_eq!(safe_label("../unsafe label"), "___unsafe_label");
    assert_eq!(safe_label(&"a".repeat(80)).len(), 64);
}

#[cfg(unix)]
#[test]
fn symlink_directory_is_rejected() {
    use std::os::unix::fs::symlink;

    let root = fixture();
    let target = root.join("target");
    std::fs::create_dir(&target).unwrap();
    let link = root.join("link");
    symlink(&target, &link).unwrap();

    let error = prepare_directory(&link).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::Other);
    std::fs::remove_dir_all(root).unwrap();
}
