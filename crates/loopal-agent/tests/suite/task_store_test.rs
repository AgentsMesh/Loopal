use loopal_agent::task_store::{TaskPatch, TaskStore};
use loopal_agent::types::TaskStatus;

#[test]
fn test_create_and_get_task() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());

    let task = store.create("Test task", "A description");
    assert_eq!(task.subject, "Test task");
    assert_eq!(task.status, TaskStatus::Pending);
    assert!(task.owner.is_none());

    let fetched = store.get(&task.id).unwrap();
    assert_eq!(fetched.subject, "Test task");
}

#[test]
fn test_list_excludes_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());

    let t1 = store.create("Task 1", "desc");
    let _t2 = store.create("Task 2", "desc");

    store.update(
        &t1.id,
        TaskPatch {
            status: Some(TaskStatus::Deleted),
            ..Default::default()
        },
    );

    let tasks = store.list();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].subject, "Task 2");
}

#[test]
fn test_update_status_and_owner() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());

    let task = store.create("Task", "desc");
    let updated = store
        .update(
            &task.id,
            TaskPatch {
                status: Some(TaskStatus::InProgress),
                owner: Some(Some("agent-1".to_string())),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
    assert_eq!(updated.owner.as_deref(), Some("agent-1"));
}

#[test]
fn test_add_blocked_by() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());

    let t1 = store.create("Task 1", "");
    let t2 = store.create("Task 2", "");

    let updated = store
        .update(
            &t2.id,
            TaskPatch {
                add_blocked_by: vec![t1.id.clone()],
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(updated.blocked_by, vec![t1.id]);
}

#[test]
fn test_update_nonexistent_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());
    assert!(store.update("999", TaskPatch::default()).is_none());
}

#[test]
fn test_persistence_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    {
        TaskStore::new(path.clone()).create("Persisted", "data");
    }

    let tasks = TaskStore::new(path).list();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].subject, "Persisted");
}

#[test]
fn test_auto_increment_ids() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());

    let t1 = store.create("A", "");
    let t2 = store.create("B", "");
    let t3 = store.create("C", "");

    let id1: u64 = t1.id.parse().unwrap();
    let id2: u64 = t2.id.parse().unwrap();
    let id3: u64 = t3.id.parse().unwrap();
    assert!(id1 < id2 && id2 < id3);
}

#[test]
fn test_subscribe_notifies_on_create() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());
    let mut rx = store.subscribe();
    store.create("Task", "desc");
    assert!(rx.try_recv().is_ok(), "should receive notification on create");
}

#[test]
fn test_subscribe_notifies_on_update() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());
    let task = store.create("Task", "desc");
    let mut rx = store.subscribe();
    store.update(
        &task.id,
        TaskPatch {
            status: Some(TaskStatus::InProgress),
            ..Default::default()
        },
    );
    assert!(rx.try_recv().is_ok(), "should receive notification on update");
}

#[test]
fn test_subscribe_no_notification_on_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());
    store.create("Task", "desc");
    let mut rx = store.subscribe();
    let _ = store.list();
    let _ = store.get("1");
    assert!(rx.try_recv().is_err(), "reads should not notify");
}

#[test]
fn test_list_excludes_deleted_after_create() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());
    let t1 = store.create("Keep", "");
    store.create("Delete me", "");
    store.update(
        "2",
        TaskPatch {
            status: Some(TaskStatus::Deleted),
            ..Default::default()
        },
    );
    let tasks = store.list();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, t1.id);
}

#[test]
fn test_list_returns_all_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());
    store.create("Pending", "");
    store.create("Active", "");
    store.create("Done", "");
    store.update(
        "2",
        TaskPatch {
            status: Some(TaskStatus::InProgress),
            active_form: Some("Building".into()),
            ..Default::default()
        },
    );
    store.update(
        "3",
        TaskPatch {
            status: Some(TaskStatus::Completed),
            ..Default::default()
        },
    );
    let tasks = store.list();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].status, TaskStatus::Pending);
    assert_eq!(tasks[1].status, TaskStatus::InProgress);
    assert_eq!(tasks[1].active_form.as_deref(), Some("Building"));
    assert_eq!(tasks[2].status, TaskStatus::Completed);
}
