use loopal_backend::fs::{ExpectedContent, write_file, write_file_if_unchanged};

#[tokio::test]
async fn concurrent_atomic_writes_use_independent_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shared.txt");
    let writes = (0..32).map(|index| {
        let path = path.clone();
        tokio::spawn(async move { write_file(&path, &format!("value-{index}")).await })
    });
    for task in writes {
        task.await.unwrap().unwrap();
    }
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.starts_with("value-"));
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".loopal.tmp"))
        .collect();
    assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
}

#[tokio::test]
async fn guarded_write_rechecks_content_before_rename() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("guarded.txt");
    std::fs::write(&path, "newer").unwrap();
    let result = write_file_if_unchanged(&path, "replacement", ExpectedContent::Bytes(b"stale"))
        .await
        .unwrap();
    assert!(result.is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "newer");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
