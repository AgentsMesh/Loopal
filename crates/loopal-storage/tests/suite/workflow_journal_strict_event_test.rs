use std::io::Write;

use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptFailure, WorkflowEventPayload, WorkflowFailureClass,
    WorkflowOutput,
};

use crate::workflow_journal_support::*;

#[test]
fn strict_event_and_common_conversion_matrix_replays() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    write_init(&path);
    let payloads = vec![
        serde_json::json!({
            "type": "dispatch_intended", "node_id": "output", "attempt_id": "watt_dispatch",
            "capability_digest": format!("sha256:{}", "11".repeat(32))
        }),
        serde_json::json!({
            "type": "attempt_bound", "node_id": "output", "attempt_id": "watt_bound",
            "agent": {"hub": ["hub"], "agent": "worker"}
        }),
        serde_json::json!({
            "type": "attempt_running", "node_id": "output", "attempt_id": "watt_running"
        }),
        serde_json::json!({
            "type": "attempt_succeeded", "node_id": "output", "attempt_id": "watt_text",
            "completion": {"reason": "done", "result": "ok"},
            "output": {"type": "text", "value": "answer"}
        }),
        serde_json::json!({
            "type": "attempt_succeeded", "node_id": "output", "attempt_id": "watt_json",
            "completion": {"reason": "done", "result": null},
            "output": {"type": "json", "value": {"ok": true}}
        }),
        failed("watt_transient", "transient_before_execution", "retry"),
        failed("watt_ambiguous", "ambiguous_execution", "unknown"),
        failed("watt_permanent", "permanent", "boom"),
        serde_json::json!({"type": "cancel_requested", "reason": "stop"}),
        serde_json::json!({
            "type": "attempt_cancelled", "node_id": "output",
            "attempt_id": "watt_cancelled", "reason": "stop"
        }),
        serde_json::json!({
            "type": "run_deadline_exceeded",
            "failure": {"class": "permanent", "reason": "deadline"}
        }),
        serde_json::json!({
            "type": "attempt_stop_requested", "node_id": "output",
            "attempt_id": "watt_stopping", "reason": "operator"
        }),
    ];
    let events: Vec<_> = payloads
        .into_iter()
        .enumerate()
        .map(|(index, payload)| {
            serde_json::json!({
                "run_id": "wrun_test",
                "revision": index + 1,
                "occurred_at_unix_ms": 101 + index,
                "payload": payload,
            })
        })
        .collect();
    let commit = serde_json::json!({
        "kind": "commit", "version": 1, "run_id": "wrun_test", "events": events
    });
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(format!("{commit}\n").as_bytes())
        .unwrap();

    let events = journal(&temp).replay().unwrap().commits.remove(0).events;
    assert!(matches!(
        &events[1].payload,
        WorkflowEventPayload::AttemptBound { agent, .. }
            if agent == &QualifiedAddress::remote(["hub"], "worker")
    ));
    assert!(matches!(
        &events[3].payload,
        WorkflowEventPayload::AttemptSucceeded {
            output: Some(WorkflowOutput::Text(value)), ..
        } if value == "answer"
    ));
    assert!(matches!(
        &events[4].payload,
        WorkflowEventPayload::AttemptSucceeded {
            output: Some(WorkflowOutput::Json(value)), ..
        } if value == &serde_json::json!({"ok": true})
    ));
    let classes: Vec<_> = events[5..8]
        .iter()
        .map(|event| match &event.payload {
            WorkflowEventPayload::AttemptFailed { failure, .. } => failure.clone(),
            payload => panic!("unexpected payload: {payload:?}"),
        })
        .collect();
    assert_eq!(
        classes,
        [
            failure(WorkflowFailureClass::TransientBeforeExecution, "retry"),
            failure(WorkflowFailureClass::AmbiguousExecution, "unknown"),
            failure(WorkflowFailureClass::Permanent, "boom"),
        ]
    );
    assert!(matches!(
        &events[10].payload,
        WorkflowEventPayload::RunDeadlineExceeded { failure }
            if failure.reason == "deadline"
    ));
    assert!(matches!(
        &events[11].payload,
        WorkflowEventPayload::AttemptStopRequested { reason, .. }
            if reason == "operator"
    ));
}

fn failed(attempt_id: &str, class: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "attempt_failed", "node_id": "output", "attempt_id": attempt_id,
        "completion": {"reason": "failed", "result": null},
        "failure": {"class": class, "reason": reason}
    })
}

fn failure(class: WorkflowFailureClass, reason: &str) -> WorkflowAttemptFailure {
    WorkflowAttemptFailure {
        class,
        reason: reason.into(),
    }
}
