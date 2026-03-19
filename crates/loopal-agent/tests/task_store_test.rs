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

    store.update(&t1.id, TaskPatch {
        status: Some(TaskStatus::Deleted),
        ..Default::default()
    });

    let tasks = store.list();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].subject, "Task 2");
}

#[test]
fn test_update_status_and_owner() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());

    let task = store.create("Task", "desc");
    let updated = store.update(&task.id, TaskPatch {
        status: Some(TaskStatus::InProgress),
        owner: Some(Some("agent-1".to_string())),
        ..Default::default()
    }).unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
    assert_eq!(updated.owner.as_deref(), Some("agent-1"));
}

#[test]
fn test_add_blocked_by() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::new(dir.path().to_path_buf());

    let t1 = store.create("Task 1", "");
    let t2 = store.create("Task 2", "");

    let updated = store.update(&t2.id, TaskPatch {
        add_blocked_by: vec![t1.id.clone()],
        ..Default::default()
    }).unwrap();

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
    { TaskStore::new(path.clone()).create("Persisted", "data"); }

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
