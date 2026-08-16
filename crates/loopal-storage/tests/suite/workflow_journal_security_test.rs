use loopal_protocol::{WorkflowEvent, WorkflowEventPayload};
use loopal_storage::WorkflowJournalError;

use crate::workflow_journal_support::*;

const PLAINTEXT_SENTINEL: &str = "S3NTINEL-plaintext-never-write";

#[test]
fn path_run_id_must_match_snapshot_event_commit_and_request_context() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    let mut wrong_snapshot = snapshot();
    wrong_snapshot.id = "wrun_other".into();
    assert!(matches!(
        journal.append_init(wrong_snapshot),
        Err(WorkflowJournalError::RunIdMismatch { .. })
    ));

    let mut wrong_event = event(1);
    wrong_event.run_id = "wrun_other".into();
    assert!(matches!(
        journal.append_commit(vec![wrong_event], None),
        Err(WorkflowJournalError::RunIdMismatch { .. })
    ));

    let mut wrong_request = request();
    wrong_request.response["run"]["id"] = serde_json::json!("wrun_other");
    let error = journal
        .append_commit(Vec::new(), Some(wrong_request))
        .unwrap_err();
    assert!(
        matches!(error, WorkflowJournalError::RunIdMismatch { .. }),
        "{error:?}"
    );
}

#[test]
fn journal_wire_has_no_execution_generation_or_authority_fields() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    journal
        .append_commit(vec![event(1)], Some(request()))
        .unwrap();
    let content = std::fs::read_to_string(path(&temp)).unwrap();
    for forbidden in [
        "AgentExecutionRef",
        "connection_generation",
        "routing_generation",
        "permission_override",
        "sandbox_override",
        "artifact",
        "expanded_input",
    ] {
        assert!(!content.contains(forbidden), "found {forbidden}");
    }
}

#[test]
fn placeholder_is_preserved_and_known_plaintext_is_absent() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    journal
        .append_commit(vec![event(1)], Some(request()))
        .unwrap();
    let content = std::fs::read_to_string(path(&temp)).unwrap();
    assert!(content.contains("<secret_ref:token>"));
    assert!(!content.contains(PLAINTEXT_SENTINEL));
}

#[test]
fn request_only_commit_still_carries_path_run_id() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    journal.append_commit(Vec::new(), Some(request())).unwrap();
    let value: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(path(&temp))
            .unwrap()
            .lines()
            .nth(1)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["run_id"], "wrun_test");
}

#[test]
fn every_event_context_is_checked() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    let events = vec![
        event(1),
        WorkflowEvent {
            run_id: "wrun_other".into(),
            revision: 2,
            occurred_at_unix_ms: 102,
            payload: WorkflowEventPayload::RunStarted,
        },
    ];
    assert!(matches!(
        journal.append_commit(events, None),
        Err(WorkflowJournalError::RunIdMismatch { .. })
    ));
}
