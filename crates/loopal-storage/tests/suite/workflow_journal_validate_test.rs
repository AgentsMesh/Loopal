use loopal_protocol::{
    MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES, WorkflowAttemptCapability, WorkflowAttemptSnapshot,
    WorkflowAttemptState, WorkflowEvent, WorkflowOutput, WorkflowRequestRecord,
};
use loopal_storage::WorkflowJournalError;

use crate::workflow_journal_support::*;

#[test]
fn empty_and_noncontiguous_commits_fail_before_io() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    journal.append_init(snapshot()).unwrap();
    let before = std::fs::read(path(&temp)).unwrap();

    assert!(matches!(
        journal.append_commit(Vec::new(), None),
        Err(WorkflowJournalError::Corruption { .. })
    ));
    assert!(matches!(
        journal.append_commit(vec![event(1), event(3)], None),
        Err(WorkflowJournalError::Corruption { .. })
    ));
    assert_eq!(std::fs::read(path(&temp)).unwrap(), before);
}

#[test]
fn invalid_request_identity_and_payload_size_fail_before_io() {
    let invalid = WorkflowRequestRecord {
        request_id: "".into(),
        ..request()
    };
    let oversized = WorkflowRequestRecord {
        request_id: "wreq_large".into(),
        operation: "get".into(),
        payload: serde_json::json!("x".repeat(MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES + 1)),
        response: serde_json::json!({"run": null}),
    };
    for request in [invalid, oversized] {
        let temp = tempfile::tempdir().unwrap();
        assert!(matches!(
            journal(&temp).append_init_with_request(snapshot(), Some(request)),
            Err(WorkflowJournalError::Corruption { .. })
        ));
        assert!(!path(&temp).exists());
    }
}

#[test]
fn invalid_initial_snapshot_shapes_fail_before_io() {
    let mut attempt = snapshot();
    attempt.attempts.push(WorkflowAttemptSnapshot {
        id: "watt_one".into(),
        node_id: "output".into(),
        capability_digest: WorkflowAttemptCapability::parse("11".repeat(32))
            .unwrap()
            .digest(),
        dispatched_at_unix_ms: 101,
        state: WorkflowAttemptState::Dispatching,
        agent: None,
        entered_running: false,
        completion: None,
        failure: None,
        output: None,
    });
    let mut result = snapshot();
    result.result = Some(WorkflowOutput::Text("unexpected".into()));
    let mut timestamps = snapshot();
    timestamps.updated_at_unix_ms += 1;
    let mut node_count = snapshot();
    node_count.nodes.clear();
    let mut node_state = snapshot();
    node_state.nodes[0].attempt_count = 1;

    for value in [attempt, result, timestamps, node_count, node_state] {
        let temp = tempfile::tempdir().unwrap();
        assert!(matches!(
            journal(&temp).append_init(value),
            Err(WorkflowJournalError::Corruption { .. })
        ));
        assert!(!path(&temp).exists());
    }
}

#[test]
fn invalid_initial_events_fail_before_io() {
    let wrong_run = WorkflowEvent {
        run_id: "wrun_other".into(),
        ..event(1)
    };
    for events in [vec![event(2)], vec![event(1), event(3)], vec![wrong_run]] {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            journal(&temp)
                .append_init_with_events(snapshot(), events, Some(start_request()))
                .is_err()
        );
        assert!(!path(&temp).exists());
    }
}

#[test]
fn invalid_actual_run_id_is_distinct_from_mismatch() {
    let temp = tempfile::tempdir().unwrap();
    let mut run = snapshot();
    run.id = "".into();

    assert!(matches!(
        journal(&temp).append_init(run),
        Err(WorkflowJournalError::InvalidRunId(actual)) if actual.is_empty()
    ));
    assert!(!path(&temp).exists());
}
