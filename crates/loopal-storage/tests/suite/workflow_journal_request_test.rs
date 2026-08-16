use loopal_protocol::{WorkflowRequestRecord, WorkflowRunSummary};
use loopal_storage::WorkflowJournalError;

use crate::workflow_journal_support::*;

fn record(
    request_id: &str,
    operation: &str,
    payload: serde_json::Value,
    response: serde_json::Value,
) -> WorkflowRequestRecord {
    WorkflowRequestRecord {
        request_id: request_id.into(),
        operation: operation.into(),
        payload,
        response,
    }
}

#[test]
fn start_wait_and_cancel_request_shapes_persist() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    let run = snapshot();
    let summary = WorkflowRunSummary::from(&run);
    let start = record(
        "wreq_start",
        "start",
        serde_json::json!({"request_id": "wreq_start", "spec": spec("goal")}),
        serde_json::json!({"summary": summary}),
    );
    journal
        .append_init_with_request(run.clone(), Some(start.clone()))
        .unwrap();

    let wait = record(
        "wreq_wait",
        "wait",
        serde_json::json!({
            "request_id": "wreq_wait",
            "run_id": "wrun_test",
            "after_revision": 0,
            "timeout_ms": 1,
        }),
        serde_json::json!({"status": "timed_out", "run": null}),
    );
    journal
        .append_commit(Vec::new(), Some(wait.clone()))
        .unwrap();

    let cancel = record(
        "wreq_cancel",
        "cancel",
        serde_json::json!({
            "request_id": "wreq_cancel",
            "run_id": "wrun_test",
            "reason": "stop",
        }),
        serde_json::json!({
            "summary": WorkflowRunSummary::from(&run),
            "already_terminal": false,
        }),
    );
    journal
        .append_commit(Vec::new(), Some(cancel.clone()))
        .unwrap();

    let replay = journal.replay().unwrap();
    assert_eq!(replay.init.unwrap().request, Some(start));
    assert_eq!(replay.commits[0].request, Some(wait));
    assert_eq!(replay.commits[1].request, Some(cancel));
}

#[test]
fn wait_accepts_every_status_and_optional_snapshot() {
    for (index, status) in ["changed", "terminal", "timed_out", "not_found"]
        .into_iter()
        .enumerate()
    {
        let temp = tempfile::tempdir().unwrap();
        let journal = journal(&temp);
        journal.append_init(snapshot()).unwrap();
        let request_id = format!("wreq_wait_{index}");
        let run = (status != "not_found").then(snapshot);
        let request = record(
            &request_id,
            "wait",
            serde_json::json!({
                "request_id": request_id,
                "run_id": "wrun_test",
                "after_revision": 0,
                "timeout_ms": 1,
            }),
            serde_json::json!({"status": status, "run": run}),
        );
        journal.append_commit(Vec::new(), Some(request)).unwrap();
    }
}

#[test]
fn unsupported_operation_and_request_id_mismatch_fail_closed() {
    let bad = [
        record(
            "wreq_unknown",
            "resume",
            serde_json::json!({}),
            serde_json::json!({}),
        ),
        record(
            "wreq_get",
            "get",
            serde_json::json!({"request_id": "wreq_other", "run_id": "wrun_test"}),
            serde_json::json!({"run": null}),
        ),
    ];
    for request in bad {
        let temp = tempfile::tempdir().unwrap();
        let journal = journal(&temp);
        assert!(matches!(
            journal.append_init_with_request(snapshot(), Some(request)),
            Err(WorkflowJournalError::Corruption { .. })
        ));
        assert!(!path(&temp).exists());
    }
}
