use loopal_error::StorageError;

use super::file_io::{existing_matches, read_regular_bounded};

#[tokio::test]
async fn bounded_read_rejects_a_directory() {
    let temp = tempfile::tempdir().unwrap();
    assert!(matches!(
        read_regular_bounded(temp.path(), 16).await,
        Err(StorageError::ResourceIntegrity)
    ));
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
