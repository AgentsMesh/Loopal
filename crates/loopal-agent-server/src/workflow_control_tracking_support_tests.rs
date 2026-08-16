use loopal_protocol::{WorkflowRunSummary, WorkflowStartRequest, WorkflowStartResponse};

pub(super) fn request(id: &str) -> WorkflowStartRequest {
    serde_json::from_value(serde_json::json!({
        "request_id": id,
        "spec": {
            "version": 1,
            "run_goal": "track workflow",
            "nodes": [{"id": "worker", "dependencies": [], "task": "work", "worker_profile": "default"}],
            "limits": {"max_nodes": 1, "max_parallel": 1, "max_attempts": 1, "run_deadline_ms": 1000, "attempt_timeout_ms": 500, "max_output_bytes": 1024},
            "output_node": "worker",
            "output_contract": {"type": "text", "max_bytes": 1024}
        }
    }))
    .unwrap()
}

pub(super) fn response(id: &str) -> WorkflowStartResponse {
    WorkflowStartResponse {
        summary: summary(id),
    }
}

pub(super) fn summary(id: &str) -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: id.into(),
        run_goal: "track workflow".into(),
        state: loopal_protocol::WorkflowRunState::Planned,
        revision: 0,
        output_node: "worker".into(),
        counts: loopal_protocol::WorkflowStateCounts {
            pending: 1,
            ready: 0,
            active: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        },
        created_at_unix_ms: 1,
        updated_at_unix_ms: 1,
    }
}
