use loopal_storage::TurnEventStore;

#[test]
fn empty_and_whitespace_only_event_logs_load_without_events() {
    for bytes in [b"".as_slice(), b" \n\n\t\n".as_slice()] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sessions/session/turns.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        let store = TurnEventStore::with_base_dir(temp.path().to_path_buf());

        assert!(store.load_events("session").unwrap().is_empty());
    }
}
