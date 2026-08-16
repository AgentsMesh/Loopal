use loopal_error::StorageError;

use super::file_io::{
    read_regular_bounded, replace_file, replace_retry_delay, retryable_verification_error,
};

#[cfg(unix)]
use super::file_io::existing_matches;

#[tokio::test]
async fn bounded_read_rejects_a_directory() {
    let temp = tempfile::tempdir().unwrap();
    assert!(matches!(
        read_regular_bounded(temp.path(), 16).await,
        Err(StorageError::ResourceIntegrity)
    ));
}

#[test]
fn replace_retry_delay_is_exponential_and_capped() {
    assert_eq!(replace_retry_delay(0), std::time::Duration::from_millis(1));
    assert_eq!(replace_retry_delay(3), std::time::Duration::from_millis(8));
    assert_eq!(
        replace_retry_delay(usize::MAX),
        std::time::Duration::from_millis(32)
    );
}

#[test]
fn non_retryable_verification_errors_are_rejected() {
    let io_error = StorageError::Io(std::io::Error::other("not retryable"));
    assert!(!retryable_verification_error(&io_error));
    assert!(!retryable_verification_error(
        &StorageError::ResourceIntegrity
    ));
}

#[tokio::test]
async fn failed_replace_accepts_an_existing_matching_winner() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing-temp");
    let target = temp.path().join("resource");
    std::fs::write(&target, b"expected").unwrap();

    replace_file(&missing, &target, b"expected").await.unwrap();

    assert_eq!(std::fs::read(target).unwrap(), b"expected");
}

// This invalid-temp fixture relies on Unix directory rename/unlink semantics.
#[cfg(unix)]
#[tokio::test]
async fn matching_winner_reports_loser_cleanup_failure() {
    let root = tempfile::tempdir().unwrap();
    let temp = root.path().join("invalid-temp-directory");
    let target = root.path().join("resource");
    std::fs::create_dir(&temp).unwrap();
    std::fs::write(&target, b"expected").unwrap();

    assert!(matches!(
        replace_file(&temp, &target, b"expected").await,
        Err(StorageError::Io(_))
    ));
    assert!(temp.is_dir());
    assert_eq!(std::fs::read(target).unwrap(), b"expected");
}

#[tokio::test]
async fn failed_replace_does_not_accept_mismatched_content() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing-temp");
    let target = temp.path().join("resource");
    std::fs::write(&target, b"tampered").unwrap();

    let error = replace_file(&missing, &target, b"expected")
        .await
        .unwrap_err();

    assert!(matches!(error, StorageError::Io(_)));
    assert_eq!(std::fs::read(target).unwrap(), b"tampered");
}

#[tokio::test]
async fn failed_replace_rejects_a_non_regular_winner() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing-temp");
    let target = temp.path().join("resource");
    std::fs::create_dir(&target).unwrap();

    assert!(matches!(
        replace_file(&missing, &target, b"expected").await,
        Err(StorageError::ResourceIntegrity)
    ));
}

#[cfg(windows)]
#[tokio::test]
async fn matching_locked_winner_resolves_replace_competition_and_cleans_temp() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    let root = tempfile::tempdir().unwrap();
    let temp = root.path().join("prepared-temp");
    let target = root.path().join("resource");
    std::fs::write(&temp, b"expected").unwrap();
    std::fs::write(&target, b"expected").unwrap();
    let locked_target = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&target)
        .unwrap();

    replace_file(&temp, &target, b"expected").await.unwrap();

    assert!(!temp.exists());
    assert_eq!(std::fs::read(&target).unwrap(), b"expected");
    drop(locked_target);
}

#[cfg(windows)]
#[tokio::test]
async fn matching_winner_reports_locked_temp_cleanup_failure() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    let root = tempfile::tempdir().unwrap();
    let temp = root.path().join("prepared-temp");
    let target = root.path().join("resource");
    std::fs::write(&temp, b"expected").unwrap();
    std::fs::write(&target, b"expected").unwrap();
    let locked_temp = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&temp)
        .unwrap();

    assert!(matches!(
        replace_file(&temp, &target, b"expected").await,
        Err(StorageError::Io(_))
    ));
    assert!(temp.exists());
    assert_eq!(std::fs::read(&target).unwrap(), b"expected");
    drop(locked_temp);
}

#[cfg(windows)]
#[tokio::test]
async fn mismatched_locked_winner_fails_closed_after_bounded_retries() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    let root = tempfile::tempdir().unwrap();
    let temp = root.path().join("prepared-temp");
    let target = root.path().join("resource");
    std::fs::write(&temp, b"expected").unwrap();
    std::fs::write(&target, b"tampered").unwrap();
    let locked_target = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&target)
        .unwrap();

    assert!(matches!(
        replace_file(&temp, &target, b"expected").await,
        Err(StorageError::Io(_))
    ));
    assert!(temp.exists());
    assert_eq!(std::fs::read(&target).unwrap(), b"tampered");
    drop(locked_target);
}

#[cfg(unix)]
#[tokio::test]
async fn existing_match_propagates_nonmissing_open_errors() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("private");
    std::fs::create_dir(&parent).unwrap();
    let path = parent.join("resource");
    std::fs::write(&path, b"value").unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o000)).unwrap();

    let result = existing_matches(&path, b"value").await;

    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(result, Err(StorageError::Io(_))));
}
