//! Unit tests for `FileScopedCronStore` (session-scoped cron storage).

use loopal_scheduler::{
    FileScopedCronStore, PersistError, PersistedTask, SessionScopedCronStorage,
};
use tempfile::tempdir;

fn task(id: &str) -> PersistedTask {
    PersistedTask {
        id: id.into(),
        cron: "*/5 * * * *".into(),
        prompt: "ping".into(),
        recurring: true,
        created_at_unix_ms: 1_700_000_000_000,
        last_fired_unix_ms: None,
    }
}

#[tokio::test]
async fn load_returns_empty_for_unknown_session() {
    let dir = tempdir().unwrap();
    let store = FileScopedCronStore::new(dir.path().to_path_buf());
    let out = store.load("nope-session").await.unwrap();
    assert!(out.is_empty());
}

#[tokio::test]
async fn save_then_load_isolates_sessions() {
    let dir = tempdir().unwrap();
    let store = FileScopedCronStore::new(dir.path().to_path_buf());
    store.save_all("alpha", &[task("a1")]).await.unwrap();
    store
        .save_all("beta", &[task("b1"), task("b2")])
        .await
        .unwrap();
    let alpha = store.load("alpha").await.unwrap();
    let beta = store.load("beta").await.unwrap();
    assert_eq!(alpha.len(), 1);
    assert_eq!(alpha[0].id, "a1");
    assert_eq!(beta.len(), 2);
}

#[tokio::test]
async fn save_creates_session_subdir() {
    let dir = tempdir().unwrap();
    let store = FileScopedCronStore::new(dir.path().to_path_buf());
    store.save_all("sid-x", &[task("x")]).await.unwrap();
    let cron_path = dir.path().join("sid-x").join("cron.json");
    assert!(cron_path.exists());
}

#[tokio::test]
async fn save_overwrites_previous_set_for_same_session() {
    let dir = tempdir().unwrap();
    let store = FileScopedCronStore::new(dir.path().to_path_buf());
    store.save_all("s", &[task("a"), task("b")]).await.unwrap();
    store.save_all("s", &[task("c")]).await.unwrap();
    let back = store.load("s").await.unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].id, "c");
}

#[tokio::test]
async fn load_quarantines_corrupt_session_file() {
    let dir = tempdir().unwrap();
    let session_dir = dir.path().join("broken");
    tokio::fs::create_dir_all(&session_dir).await.unwrap();
    let cron_path = session_dir.join("cron.json");
    tokio::fs::write(&cron_path, b"not json").await.unwrap();
    let store = FileScopedCronStore::new(dir.path().to_path_buf());
    let out = store.load("broken").await.unwrap();
    assert!(out.is_empty());
    assert!(!cron_path.exists());
    let mut entries = tokio::fs::read_dir(&session_dir).await.unwrap();
    let mut quarantined = false;
    while let Some(e) = entries.next_entry().await.unwrap() {
        if e.file_name()
            .to_string_lossy()
            .starts_with("cron.json.bad-")
        {
            quarantined = true;
            break;
        }
    }
    assert!(quarantined);
}

#[tokio::test]
async fn legacy_cron_json_loads_unchanged() {
    // Backward-compat: a fixture matching the v1 schema written by the
    // legacy `FileDurableStore` must decode identically here.
    let dir = tempdir().unwrap();
    let session_dir = dir.path().join("legacy");
    tokio::fs::create_dir_all(&session_dir).await.unwrap();
    let payload = br#"{"version":1,"tasks":[{"id":"abc12345","cron":"0 9 * * 1-5","prompt":"hello","recurring":true,"created_at_unix_ms":1700000000000}]}"#;
    tokio::fs::write(session_dir.join("cron.json"), payload)
        .await
        .unwrap();
    let store = FileScopedCronStore::new(dir.path().to_path_buf());
    let out = store.load("legacy").await.unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, "abc12345");
    assert_eq!(out[0].prompt, "hello");
    assert!(out[0].last_fired_unix_ms.is_none());
}

#[tokio::test]
async fn root_accessor_returns_input() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = FileScopedCronStore::new(root.clone());
    assert_eq!(store.root(), root.as_path());
    let _: PersistError = PersistError::BadCron("touch enum to avoid unused import".into());
}
