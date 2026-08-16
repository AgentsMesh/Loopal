use super::create;
#[cfg(unix)]
use super::{ensure_private_directory, ensure_private_file};

fn unique_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("loopal-{label}-{}", uuid::Uuid::new_v4()))
}

#[tokio::test]
async fn create_rejects_an_existing_path() {
    let path = unique_path("existing-log");
    std::fs::write(&path, b"existing").unwrap();

    let error = create(&path).await.unwrap_err();
    assert!(error.to_string().contains("create log file"));
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn private_directory_inspection_fails_for_missing_path() {
    let path = unique_path("missing-log-dir");

    let error = ensure_private_directory(&path).await.unwrap_err();
    assert!(error.to_string().contains("inspect log directory"));
}

#[cfg(unix)]
#[tokio::test]
async fn directory_handle_is_rejected_as_log_file() {
    let path = unique_path("directory-handle");
    std::fs::create_dir(&path).unwrap();
    let file = tokio::fs::File::open(&path).await.unwrap();

    let error = ensure_private_file(&path, &file).await.unwrap_err();
    assert!(error.to_string().contains("not a regular file"));
    std::fs::remove_dir(path).unwrap();
}
