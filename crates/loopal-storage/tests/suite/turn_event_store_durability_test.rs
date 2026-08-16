use chrono::Utc;
use loopal_storage::TurnEventStore;
use loopal_turn::{TurnEvent, TurnId, TurnTrigger};

fn started(id: &str) -> TurnEvent {
    TurnEvent::TurnStarted {
        turn_id: TurnId::from_string(id),
        started_at: Utc::now(),
        trigger: TurnTrigger::Resume,
    }
}

fn path(root: &std::path::Path, session: &str) -> std::path::PathBuf {
    root.join("sessions").join(session).join("turns.jsonl")
}

#[test]
fn durable_append_writes_complete_newline_terminated_record() {
    let temp = tempfile::tempdir().unwrap();
    let store = TurnEventStore::with_base_dir(temp.path().to_path_buf());
    store
        .append_event_durable("session-durable", &started("t-one"))
        .unwrap();

    let bytes = std::fs::read(path(temp.path(), "session-durable")).unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(store.load_events("session-durable").unwrap().len(), 1);
    store.sync_events("session-durable").unwrap();
}

#[test]
fn torn_tail_is_removed_without_losing_complete_records() {
    let temp = tempfile::tempdir().unwrap();
    let store = TurnEventStore::with_base_dir(temp.path().to_path_buf());
    let session = "session-torn";
    store
        .append_event_durable(session, &started("t-complete"))
        .unwrap();
    let file = path(temp.path(), session);
    use std::io::Write as _;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap()
        .write_all(b"{\"type\":\"TurnStarted\"")
        .unwrap();

    let events = store.load_events(session).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(std::fs::read(&file).unwrap().last(), Some(&b'\n'));
    store
        .append_event_durable(session, &started("t-after-repair"))
        .unwrap();
    assert_eq!(store.load_events(session).unwrap().len(), 2);
}

#[test]
fn syncing_absent_event_log_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let store = TurnEventStore::with_base_dir(temp.path().to_path_buf());
    assert!(store.sync_events("missing-session").is_err());
}
