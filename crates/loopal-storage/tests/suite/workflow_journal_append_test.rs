use loopal_protocol::{WorkflowEventPayload, WorkflowRequestRecord};
use loopal_storage::{MAX_WORKFLOW_EVENTS_PER_COMMIT, WorkflowJournalError, WorkflowJournalLimit};

use crate::workflow_journal_support::*;

#[test]
fn init_and_commit_replay_in_order() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    journal
        .append_commit(vec![event(1), event(2)], Some(request()))
        .unwrap();

    let replay = journal.replay().unwrap();
    assert_eq!(replay.init.unwrap().snapshot, snapshot());
    assert_eq!(replay.commits.len(), 1);
    assert_eq!(replay.commits[0].events, vec![event(1), event(2)]);
    assert_eq!(replay.commits[0].request, Some(request()));
}

#[test]
fn init_can_atomically_persist_start_request() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal
        .append_init_with_request(snapshot(), Some(request()))
        .unwrap();

    let replay = journal.replay().unwrap();
    assert_eq!(replay.init.unwrap().request, Some(request()));
    assert_eq!(
        std::fs::read_to_string(path(&temp))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn init_atomically_persists_initial_events_and_start_request() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal
        .append_init_with_events(snapshot(), vec![event(1)], Some(start_request()))
        .unwrap();

    let replay = journal.replay().unwrap();
    let init = replay.init.unwrap();
    assert_eq!(init.snapshot, snapshot());
    assert_eq!(init.events, vec![event(1)]);
    assert_eq!(init.request, Some(start_request()));
    assert_eq!(
        std::fs::read_to_string(path(&temp))
            .unwrap()
            .lines()
            .count(),
        1
    );

    journal.append_commit(vec![event(2)], None).unwrap();
    assert_eq!(journal.replay().unwrap().commits[0].events, vec![event(2)]);
}

#[test]
fn durable_lines_end_in_newlines_and_commit_is_one_object() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    journal
        .append_commit(vec![event(1)], Some(request()))
        .unwrap();

    let bytes = std::fs::read(path(&temp)).unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    let lines: Vec<&[u8]> = bytes.split(|byte| *byte == b'\n').collect();
    assert_eq!(lines.len(), 3);
    let commit: serde_json::Value = serde_json::from_slice(lines[1]).unwrap();
    assert_eq!(commit["events"].as_array().unwrap().len(), 1);
    assert!(commit["request"].is_object());
}

#[test]
fn invalid_append_does_not_create_or_extend_file() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    let mut wrong = snapshot();
    wrong.id = "wrun_other".into();
    assert!(matches!(
        journal.append_init(wrong),
        Err(WorkflowJournalError::RunIdMismatch { .. })
    ));
    assert!(!path(&temp).exists());

    journal.append_init(snapshot()).unwrap();
    let before = std::fs::read(path(&temp)).unwrap();
    let mut wrong_event = event(1);
    wrong_event.run_id = "wrun_other".into();
    assert!(journal.append_commit(vec![wrong_event], None).is_err());
    assert_eq!(std::fs::read(path(&temp)).unwrap(), before);
}

#[test]
fn event_count_and_request_size_fail_before_file_io() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    let events = (0..=MAX_WORKFLOW_EVENTS_PER_COMMIT)
        .map(|index| loopal_protocol::WorkflowEvent {
            revision: index as u64 + 1,
            payload: WorkflowEventPayload::CancelRequested { reason: None },
            ..event(1)
        })
        .collect();
    assert!(matches!(
        journal.append_commit(events, None),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::EventsPerCommit,
            ..
        })
    ));
    assert!(!path(&temp).exists());

    let large = WorkflowRequestRecord {
        response: serde_json::json!(
            "x".repeat(loopal_protocol::MAX_WORKFLOW_REQUEST_RESPONSE_BYTES + 1)
        ),
        ..request()
    };
    assert!(journal.append_commit(Vec::new(), Some(large)).is_err());
    assert!(!path(&temp).exists());
}
