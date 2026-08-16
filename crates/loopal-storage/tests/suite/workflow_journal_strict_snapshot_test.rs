use loopal_storage::WorkflowJournalError;

use crate::workflow_journal_support::*;

#[test]
fn strict_run_state_matrix_converts_before_validation() {
    for state in [
        "validated",
        "running",
        "cancelling",
        "succeeded",
        "failed",
        "cancelled",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut value = init_value();
        value["snapshot"]["state"] = state.into();
        value["snapshot"]["revision"] = 1.into();

        write_value(&path(&temp), &value);
        assert_corrupt(&temp);
    }
}

#[test]
fn strict_node_and_attempt_state_matrices_convert_before_validation() {
    let temp = tempfile::tempdir().unwrap();
    let mut value = init_value();
    let node_states = [
        "ready",
        "dispatching",
        "running",
        "cancelling",
        "succeeded",
        "failed",
        "cancelled",
        "skipped",
    ];
    value["snapshot"]["nodes"] = node_states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            serde_json::json!({
                "id": format!("node_{index}"),
                "dependencies": [],
                "state": state,
                "current_attempt": null,
                "attempt_count": 1,
            })
        })
        .collect::<Vec<_>>()
        .into();
    let attempt_states = [
        "dispatching",
        "running",
        "cancelling",
        "succeeded",
        "failed",
        "cancelled",
    ];
    value["snapshot"]["attempts"] = attempt_states
        .iter()
        .enumerate()
        .map(|(index, state)| attempt(index, state))
        .collect::<Vec<_>>()
        .into();
    value["snapshot"]["spec"]["output_contract"] = serde_json::json!({
        "type": "json",
        "max_bytes": 1024,
        "schema": {"type": "object"},
    });

    write_value(&path(&temp), &value);
    assert_corrupt(&temp);
}

fn init_value() -> serde_json::Value {
    serde_json::json!({
        "kind": "init",
        "version": 1,
        "snapshot": snapshot(),
    })
}

fn attempt(index: usize, state: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("watt_{index}"),
        "node_id": "output",
        "capability_digest": format!("sha256:{}", "11".repeat(32)),
        "dispatched_at_unix_ms": 200 + index,
        "state": state,
        "agent": {"hub": ["hub"], "agent": "worker"},
        "entered_running": state != "dispatching",
        "completion": {"reason": "done", "result": "result"},
        "failure": {"class": "permanent", "reason": "failed"},
        "output": {"type": "json", "value": {"index": index}},
    })
}

fn write_value(path: &std::path::Path, value: &serde_json::Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{value}\n")).unwrap();
}

fn assert_corrupt(temp: &tempfile::TempDir) {
    assert!(matches!(
        journal(temp).replay(),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}
