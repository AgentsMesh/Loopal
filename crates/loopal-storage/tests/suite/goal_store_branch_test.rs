use loopal_error::StorageError;
use loopal_storage::GoalStore;

#[test]
fn load_and_clear_propagate_nonmissing_io_errors() {
    let temp = tempfile::tempdir().unwrap();
    let store = GoalStore::with_base_dir(temp.path().to_path_buf());
    let goal_path = temp.path().join("sessions/session/goal.json");
    std::fs::create_dir_all(&goal_path).unwrap();

    assert!(matches!(store.load("session"), Err(StorageError::Io(_))));
    assert!(matches!(store.clear("session"), Err(StorageError::Io(_))));
}
