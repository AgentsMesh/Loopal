use loopal_protocol::{ThreadGoal, ThreadGoalStatus};
use loopal_storage::GoalStore;
use tempfile::TempDir;

fn store() -> (TempDir, GoalStore) {
    let tmp = TempDir::new().unwrap();
    let store = GoalStore::with_base_dir(tmp.path().to_path_buf());
    (tmp, store)
}

#[test]
fn load_returns_none_when_no_goal_file() {
    let (_tmp, store) = store();
    assert!(store.load("missing-session").unwrap().is_none());
}

#[test]
fn save_then_load_roundtrip() {
    let (_tmp, store) = store();
    let goal = ThreadGoal::new("sess-1", "Refactor X").with_token_budget(10_000);
    store.save(&goal).unwrap();

    let loaded = store.load("sess-1").unwrap().unwrap();
    assert_eq!(loaded.session_id, "sess-1");
    assert_eq!(loaded.objective, "Refactor X");
    assert_eq!(loaded.status, ThreadGoalStatus::Active);
    assert_eq!(loaded.token_budget, Some(10_000));
}

#[test]
fn save_overwrites_previous_goal() {
    let (_tmp, store) = store();
    let mut goal = ThreadGoal::new("sess-2", "First objective");
    store.save(&goal).unwrap();
    goal.objective = "Second objective".to_string();
    goal.status = ThreadGoalStatus::Paused;
    store.save(&goal).unwrap();

    let loaded = store.load("sess-2").unwrap().unwrap();
    assert_eq!(loaded.objective, "Second objective");
    assert_eq!(loaded.status, ThreadGoalStatus::Paused);
}

#[test]
fn clear_removes_goal_file() {
    let (_tmp, store) = store();
    let goal = ThreadGoal::new("sess-3", "x");
    store.save(&goal).unwrap();
    assert!(store.load("sess-3").unwrap().is_some());
    store.clear("sess-3").unwrap();
    assert!(store.load("sess-3").unwrap().is_none());
}

#[test]
fn clear_is_noop_when_goal_missing() {
    let (_tmp, store) = store();
    store.clear("never-existed").unwrap();
    assert!(store.load("never-existed").unwrap().is_none());
}

#[test]
fn save_atomic_does_not_leave_tmp_visible_under_goal_path() {
    let (_tmp, store) = store();
    let goal = ThreadGoal::new("sess-4", "atomic");
    store.save(&goal).unwrap();
    let session_dir = _tmp.path().join("sessions").join("sess-4");
    let entries: Vec<_> = std::fs::read_dir(&session_dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    assert!(
        entries.iter().any(|n| n == "goal.json"),
        "goal.json must exist after save: {entries:?}"
    );
    assert!(
        !entries.iter().any(|n| n.starts_with(".goal.json.tmp")),
        "tmp file must be renamed away after save: {entries:?}"
    );
}

#[test]
fn load_returns_serialization_error_on_corrupt_file() {
    let (tmp, store) = store();
    let session_dir = tmp.path().join("sessions").join("corrupt");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(session_dir.join("goal.json"), "not valid json").unwrap();
    let err = store.load("corrupt").unwrap_err();
    assert!(matches!(err, loopal_error::StorageError::Serialization(_)));
}

#[test]
fn save_creates_session_dir_lazily() {
    let (tmp, store) = store();
    let session_path = tmp.path().join("sessions").join("lazy");
    assert!(!session_path.exists());
    let goal = ThreadGoal::new("lazy", "create dir");
    store.save(&goal).unwrap();
    assert!(session_path.join("goal.json").exists());
}
