//! Unit tests for `FileScopedTaskStore` (session-scoped task storage).

use loopal_agent::types::{Task, TaskStatus};
use loopal_agent::{FileScopedTaskStore, SessionScopedTaskStorage};
use tempfile::tempdir;

fn task(id: &str, subject: &str) -> Task {
    Task {
        id: id.into(),
        subject: subject.into(),
        description: String::new(),
        active_form: None,
        status: TaskStatus::Pending,
        owner: None,
        blocked_by: Vec::new(),
        blocks: Vec::new(),
        metadata: serde_json::Value::Object(Default::default()),
        created_at: "2026-04-26T00:00:00Z".into(),
    }
}

#[tokio::test]
async fn load_returns_empty_for_unknown_session() {
    let dir = tempdir().unwrap();
    let store = FileScopedTaskStore::new(dir.path().to_path_buf());
    let (tasks, next_id) = store.load("unknown").await.unwrap();
    assert!(tasks.is_empty());
    assert_eq!(next_id, 1);
}

#[tokio::test]
async fn save_creates_session_subdir_and_file() {
    let dir = tempdir().unwrap();
    let store = FileScopedTaskStore::new(dir.path().to_path_buf());
    store.save_all("sid", &[task("1", "first")]).await.unwrap();
    let path = dir.path().join("sid").join("tasks").join("tasks.json");
    assert!(path.exists());
}

#[tokio::test]
async fn save_then_load_roundtrip_with_max_id_plus_one() {
    let dir = tempdir().unwrap();
    let store = FileScopedTaskStore::new(dir.path().to_path_buf());
    store
        .save_all("s", &[task("1", "first"), task("3", "third")])
        .await
        .unwrap();
    let (tasks, next_id) = store.load("s").await.unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(next_id, 4);
}

#[tokio::test]
async fn save_isolates_distinct_sessions() {
    let dir = tempdir().unwrap();
    let store = FileScopedTaskStore::new(dir.path().to_path_buf());
    store.save_all("a", &[task("1", "in a")]).await.unwrap();
    store
        .save_all("b", &[task("1", "in b"), task("2", "also b")])
        .await
        .unwrap();
    let (a, _) = store.load("a").await.unwrap();
    let (b, _) = store.load("b").await.unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].subject, "in a");
    assert_eq!(b.len(), 2);
}

#[tokio::test]
async fn save_overwrites_previous_set_for_same_session() {
    let dir = tempdir().unwrap();
    let store = FileScopedTaskStore::new(dir.path().to_path_buf());
    store.save_all("s", &[task("1", "old")]).await.unwrap();
    store.save_all("s", &[task("9", "new")]).await.unwrap();
    let (tasks, next_id) = store.load("s").await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].subject, "new");
    assert_eq!(next_id, 10);
}

#[tokio::test]
async fn load_returns_err_on_corrupt_json() {
    let dir = tempdir().unwrap();
    let session_dir = dir.path().join("broken").join("tasks");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(session_dir.join("tasks.json"), "garbage").unwrap();
    let store = FileScopedTaskStore::new(dir.path().to_path_buf());
    let err = store
        .load("broken")
        .await
        .expect_err("corrupt JSON must surface");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[tokio::test]
async fn root_accessor_returns_input() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = FileScopedTaskStore::new(root.clone());
    assert_eq!(store.root(), root.as_path());
}
