//! Tests for `InMemoryTaskStorage` — the ephemeral backend used by
//! sub-agent task stores.

use loopal_agent::types::{Task, TaskStatus};
use loopal_agent::{InMemoryTaskStorage, SessionScopedTaskStorage};

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
async fn load_unknown_session_returns_empty_with_next_id_one() {
    let storage = InMemoryTaskStorage::new();
    let (tasks, next_id) = storage.load("nope").await.unwrap();
    assert!(tasks.is_empty());
    assert_eq!(next_id, 1);
}

#[tokio::test]
async fn save_then_load_roundtrip_per_session() {
    let storage = InMemoryTaskStorage::new();
    storage
        .save_all("alpha", &[task("1", "in alpha")])
        .await
        .unwrap();
    storage
        .save_all("beta", &[task("1", "in beta"), task("2", "in beta-2")])
        .await
        .unwrap();
    let (a, _) = storage.load("alpha").await.unwrap();
    let (b, _) = storage.load("beta").await.unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].subject, "in alpha");
    assert_eq!(b.len(), 2);
}

#[tokio::test]
async fn save_overrides_previous_for_same_session() {
    let storage = InMemoryTaskStorage::new();
    storage.save_all("s", &[task("1", "old")]).await.unwrap();
    storage.save_all("s", &[task("9", "new")]).await.unwrap();
    let (tasks, next_id) = storage.load("s").await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].subject, "new");
    assert_eq!(next_id, 10);
}

#[tokio::test]
async fn next_id_reflects_max_numeric_id_plus_one() {
    let storage = InMemoryTaskStorage::new();
    storage
        .save_all(
            "s",
            &[task("3", "x"), task("7", "y"), task("non-numeric", "z")],
        )
        .await
        .unwrap();
    let (_, next_id) = storage.load("s").await.unwrap();
    assert_eq!(next_id, 8, "non-numeric IDs are skipped, max numeric=7");
}

#[tokio::test]
async fn empty_save_yields_empty_load_with_next_id_one() {
    let storage = InMemoryTaskStorage::new();
    storage.save_all("s", &[]).await.unwrap();
    let (tasks, next_id) = storage.load("s").await.unwrap();
    assert!(tasks.is_empty());
    assert_eq!(next_id, 1);
}

#[tokio::test]
async fn isolated_storage_instances_do_not_share_state() {
    let storage_a = InMemoryTaskStorage::new();
    let storage_b = InMemoryTaskStorage::new();
    storage_a
        .save_all("s", &[task("1", "a-only")])
        .await
        .unwrap();
    let (b_tasks, _) = storage_b.load("s").await.unwrap();
    assert!(
        b_tasks.is_empty(),
        "second instance must not see first's state"
    );
}
