use loopal_agent::task_patch::TaskPatch;
use loopal_agent::task_store::TaskStore;
use loopal_agent::types::TaskStatus;

const SID: &str = "test-session";

async fn make_store(dir: &std::path::Path) -> TaskStore {
    let store = TaskStore::with_sessions_root(dir.to_path_buf());
    store.switch_session(SID).await.unwrap();
    store
}

#[tokio::test]
async fn test_create_and_get_task() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;

    let task = store.create("Test task", "A description").await;
    assert_eq!(task.subject, "Test task");
    assert_eq!(task.status, TaskStatus::Pending);
    assert!(task.owner.is_none());

    let fetched = store.get(&task.id).await.unwrap();
    assert_eq!(fetched.subject, "Test task");
}

#[tokio::test]
async fn test_list_excludes_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;

    let t1 = store.create("Task 1", "desc").await;
    let _t2 = store.create("Task 2", "desc").await;

    store
        .update(
            &t1.id,
            TaskPatch {
                status: Some(TaskStatus::Deleted),
                ..Default::default()
            },
        )
        .await;

    let tasks = store.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].subject, "Task 2");
}

#[tokio::test]
async fn test_update_status_and_owner() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;

    let task = store.create("Task", "desc").await;
    let updated = store
        .update(
            &task.id,
            TaskPatch {
                status: Some(TaskStatus::InProgress),
                owner: Some(Some("agent-1".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
    assert_eq!(updated.owner.as_deref(), Some("agent-1"));
}

#[tokio::test]
async fn test_add_blocked_by() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;

    let t1 = store.create("Task 1", "").await;
    let t2 = store.create("Task 2", "").await;

    let updated = store
        .update(
            &t2.id,
            TaskPatch {
                add_blocked_by: vec![t1.id.clone()],
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.blocked_by, vec![t1.id]);
}

#[tokio::test]
async fn test_update_nonexistent_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;
    assert!(store.update("999", TaskPatch::default()).await.is_none());
}

#[tokio::test]
async fn test_persistence_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    {
        make_store(dir.path())
            .await
            .create("Persisted", "data")
            .await;
    }
    let tasks = make_store(dir.path()).await.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].subject, "Persisted");
}

#[tokio::test]
async fn test_auto_increment_ids() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;

    let t1 = store.create("A", "").await;
    let t2 = store.create("B", "").await;
    let t3 = store.create("C", "").await;

    let id1: u64 = t1.id.parse().unwrap();
    let id2: u64 = t2.id.parse().unwrap();
    let id3: u64 = t3.id.parse().unwrap();
    assert!(id1 < id2 && id2 < id3);
}

#[tokio::test]
async fn test_subscribe_notifies_on_create() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;
    let mut rx = store.subscribe();
    store.create("Task", "desc").await;
    assert!(
        rx.try_recv().is_ok(),
        "should receive notification on create"
    );
}

#[tokio::test]
async fn test_subscribe_notifies_on_update() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;
    let task = store.create("Task", "desc").await;
    let mut rx = store.subscribe();
    store
        .update(
            &task.id,
            TaskPatch {
                status: Some(TaskStatus::InProgress),
                ..Default::default()
            },
        )
        .await;
    assert!(
        rx.try_recv().is_ok(),
        "should receive notification on update"
    );
}

#[tokio::test]
async fn test_subscribe_no_notification_on_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;
    store.create("Task", "desc").await;
    let mut rx = store.subscribe();
    let _ = store.list().await;
    let _ = store.get("1").await;
    assert!(rx.try_recv().is_err(), "reads should not notify");
}

#[tokio::test]
async fn test_list_excludes_deleted_after_create() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;
    let t1 = store.create("Keep", "").await;
    store.create("Delete me", "").await;
    store
        .update(
            "2",
            TaskPatch {
                status: Some(TaskStatus::Deleted),
                ..Default::default()
            },
        )
        .await;
    let tasks = store.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, t1.id);
}

#[tokio::test]
async fn test_list_returns_all_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path()).await;
    store.create("Pending", "").await;
    store.create("Active", "").await;
    store.create("Done", "").await;
    store
        .update(
            "2",
            TaskPatch {
                status: Some(TaskStatus::InProgress),
                active_form: Some("Building".into()),
                ..Default::default()
            },
        )
        .await;
    store
        .update(
            "3",
            TaskPatch {
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .await;
    let tasks = store.list().await;
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].status, TaskStatus::Pending);
    assert_eq!(tasks[1].status, TaskStatus::InProgress);
    assert_eq!(tasks[1].active_form.as_deref(), Some("Building"));
    assert_eq!(tasks[2].status, TaskStatus::Completed);
}
