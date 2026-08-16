use loopal_backend::create_log_file_in;

#[cfg(unix)]
#[tokio::test]
async fn log_creation_repairs_directory_modes_and_creates_private_files() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("logs");
    let (first, _) = create_log_file_in(&root, "private").await.unwrap();
    let session = root.join("private");
    let bash = session.join("bash");
    for path in [&root, &session, &bash] {
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .await
            .unwrap();
    }

    let (second, _) = create_log_file_in(&root, "private").await.unwrap();
    for path in [&root, &session, &bash] {
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    for path in [first, second] {
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn log_creation_rejects_symlinked_root() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    let root = temp.path().join("root");
    std::fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, &root).unwrap();

    let error = create_log_file_in(&root, "session").await.unwrap_err();
    assert!(error.to_string().contains("not a regular directory"));
}

#[cfg(unix)]
#[tokio::test]
async fn log_creation_rejects_symlinked_session_and_bash_directories() {
    for leaf in ["session", "session/bash"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let target = temp.path().join("target");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir(&target).unwrap();
        if leaf.ends_with("/bash") {
            std::fs::create_dir(root.join("session")).unwrap();
        }
        std::os::unix::fs::symlink(&target, root.join(leaf)).unwrap();

        let error = create_log_file_in(&root, "session").await.unwrap_err();
        assert!(error.to_string().contains("not a regular directory"));
    }
}

#[tokio::test]
async fn log_creation_rejects_a_regular_file_as_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::write(&root, b"not a directory").unwrap();

    let error = create_log_file_in(&root, "session").await.unwrap_err();
    assert!(error.to_string().contains("create log directory"));
}
