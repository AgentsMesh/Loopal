use loopal_protocol::{
    MAX_WORKFLOW_REQUEST_OPERATION_BYTES, WorkflowAttemptFailure, WorkflowFailureClass,
    WorkflowNodeState, WorkflowRunState,
};
use loopal_storage::WorkflowJournalError;

use crate::workflow_journal_support::*;

fn assert_init_rejected(value: loopal_protocol::WorkflowRunSnapshot) {
    let temp = tempfile::tempdir().unwrap();
    assert!(matches!(
        journal(&temp).append_init(value),
        Err(WorkflowJournalError::Corruption { .. })
    ));
    assert!(!path(&temp).exists());
}

#[test]
fn invalid_run_id_is_rejected_before_path_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let sessions = loopal_storage::SessionStore::with_base_dir(temp.path().to_path_buf());
    assert!(matches!(
        loopal_storage::WorkflowJournal::from_session_store(&sessions, "session", "".into()),
        Err(WorkflowJournalError::InvalidRunId(actual)) if actual.is_empty()
    ));
}

#[test]
fn unsupported_journal_version_is_rejected_during_replay() {
    let temp = tempfile::tempdir().unwrap();
    let record = serde_json::json!({
        "kind": "init",
        "version": 2,
        "snapshot": snapshot(),
        "events": [],
        "request": null
    });
    let journal_path = path(&temp);
    std::fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
    std::fs::write(&journal_path, format!("{record}\n")).unwrap();

    assert!(matches!(
        journal(&temp).replay(),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

#[test]
fn empty_and_oversized_request_operations_are_rejected() {
    for operation in [
        String::new(),
        "x".repeat(MAX_WORKFLOW_REQUEST_OPERATION_BYTES + 1),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let request = loopal_protocol::WorkflowRequestRecord {
            operation,
            ..start_request()
        };
        assert!(matches!(
            journal(&temp).append_init_with_request(snapshot(), Some(request)),
            Err(WorkflowJournalError::Corruption { .. })
        ));
        assert!(!path(&temp).exists());
    }
}

#[test]
fn init_rejects_nonplanned_state_and_failure() {
    let mut state = snapshot();
    state.state = WorkflowRunState::Validated;
    let mut failure = snapshot();
    failure.failure = Some(WorkflowAttemptFailure {
        class: WorkflowFailureClass::Permanent,
        reason: "unexpected".into(),
    });

    for value in [state, failure] {
        assert_init_rejected(value);
    }
}

#[test]
fn init_rejects_each_nonplanned_node_field() {
    let mut id = snapshot();
    id.nodes[0].id = "other".into();
    let mut dependencies = snapshot();
    dependencies.nodes[0].dependencies = vec!["other".into()];
    let mut state = snapshot();
    state.nodes[0].state = WorkflowNodeState::Ready;
    let mut current_attempt = snapshot();
    current_attempt.nodes[0].current_attempt = Some("watt_unexpected".into());

    for value in [id, dependencies, state, current_attempt] {
        assert_init_rejected(value);
    }
}
