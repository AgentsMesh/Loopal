//! Unit tests for the file-backed `DurableStore`.

use loopal_scheduler::{DurableStore, FileDurableStore, PersistedTask};
use tempfile::tempdir;

fn sample_task(id: &str, cron: &str, prompt: &str) -> PersistedTask {
    PersistedTask {
        id: id.into(),
        cron: cron.into(),
        prompt: prompt.into(),
        recurring: true,
        created_at_unix_ms: 1_700_000_000_000,
        last_fired_unix_ms: None,
    }
}

#[tokio::test]
async fn load_missing_file_returns_empty() {
    let dir = tempdir().unwrap();
    let store = FileDurableStore::new(dir.path().join("cron.json"));
    let out = store.load().await.expect("load should succeed");
    assert!(out.is_empty());
}

#[tokio::test]
async fn roundtrip_preserves_fields() {
    let dir = tempdir().unwrap();
    let store = FileDurableStore::new(dir.path().join("cron.json"));
    let tasks = vec![
        sample_task("abc12345", "*/5 * * * *", "first"),
        PersistedTask {
            last_fired_unix_ms: Some(1_700_000_999_000),
            ..sample_task("def67890", "0 9 * * *", "second")
        },
    ];
    store.save_all(&tasks).await.expect("save");
    let back = store.load().await.expect("load");
    assert_eq!(back, tasks);
}

#[tokio::test]
async fn save_creates_parent_directories() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("nested").join("layer").join("cron.json");
    let store = FileDurableStore::new(nested.clone());
    store
        .save_all(&[sample_task("abc", "*/1 * * * *", "x")])
        .await
        .expect("save");
    assert!(nested.exists(), "file must be written under a created dir");
}

#[tokio::test]
async fn save_is_atomic_replace() {
    // After a successful save, the .tmp path must not linger.
    let dir = tempdir().unwrap();
    let path = dir.path().join("cron.json");
    let store = FileDurableStore::new(path.clone());
    store
        .save_all(&[sample_task("abc", "*/1 * * * *", "hi")])
        .await
        .expect("save");
    let tmp = path.with_file_name("cron.json.tmp");
    assert!(!tmp.exists(), "tmp file should be renamed away after save");
    assert!(path.exists(), "final file must exist");
}

#[tokio::test]
async fn unsupported_version_is_quarantined() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cron.json");
    tokio::fs::write(&path, br#"{"version":99,"tasks":[]}"#)
        .await
        .unwrap();
    let store = FileDurableStore::new(path.clone());
    let out = store
        .load()
        .await
        .expect("load must return Ok after quarantine");
    assert!(out.is_empty(), "quarantined payload must not surface tasks");
    assert!(!path.exists(), "original file must be renamed away");
    // The sibling ".bad-*" should now exist — one of them.
    let mut entries = tokio::fs::read_dir(dir.path()).await.unwrap();
    let mut has_bak = false;
    while let Some(e) = entries.next_entry().await.unwrap() {
        if e.file_name()
            .to_string_lossy()
            .starts_with("cron.json.bad-")
        {
            has_bak = true;
            break;
        }
    }
    assert!(has_bak, "quarantined file must exist alongside");
}

#[tokio::test]
async fn corrupt_json_is_quarantined() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cron.json");
    tokio::fs::write(&path, b"not json").await.unwrap();
    let store = FileDurableStore::new(path.clone());
    let out = store
        .load()
        .await
        .expect("load must return Ok after quarantine");
    assert!(out.is_empty());
    assert!(!path.exists());
}

#[tokio::test]
async fn path_accessor_returns_input() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cron.json");
    let store = FileDurableStore::new(path.clone());
    assert_eq!(store.path(), path.as_path());
}

#[tokio::test]
async fn save_overwrites_previous_contents() {
    let dir = tempdir().unwrap();
    let store = FileDurableStore::new(dir.path().join("cron.json"));
    store
        .save_all(&[sample_task("a", "*/5 * * * *", "first")])
        .await
        .unwrap();
    store
        .save_all(&[sample_task("b", "*/10 * * * *", "second")])
        .await
        .unwrap();
    let back = store.load().await.unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].id, "b");
}
