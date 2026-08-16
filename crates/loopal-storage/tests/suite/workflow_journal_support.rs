use loopal_protocol::*;
use loopal_storage::{SessionStore, WorkflowJournal};

pub fn journal(temp: &tempfile::TempDir) -> WorkflowJournal {
    let sessions = SessionStore::with_base_dir(temp.path().to_path_buf());
    WorkflowJournal::from_session_store(&sessions, "session-one", "wrun_test".into()).unwrap()
}

pub fn path(temp: &tempfile::TempDir) -> std::path::PathBuf {
    temp.path()
        .join("sessions/session-one/workflows/wrun_test.jsonl")
}

pub fn snapshot() -> WorkflowRunSnapshot {
    WorkflowRunSnapshot::planned(
        "wrun_test".into(),
        QualifiedAddress::local("root"),
        spec("goal <secret_ref:token>"),
        100,
    )
}

pub fn spec(goal: &str) -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: goal.into(),
        nodes: vec![WorkflowAgentNode {
            id: "output".into(),
            dependencies: Vec::new(),
            task: "answer using <secret_ref:token>".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 4_096,
        },
        output_node: "output".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}

pub fn event(revision: u64) -> WorkflowEvent {
    WorkflowEvent {
        run_id: "wrun_test".into(),
        revision,
        occurred_at_unix_ms: 100 + revision,
        payload: if revision == 1 {
            WorkflowEventPayload::SpecValidated
        } else {
            WorkflowEventPayload::RunStarted
        },
    }
}

pub fn start_request() -> WorkflowRequestRecord {
    let planned = snapshot();
    let mut validated = planned.clone();
    validated.state = WorkflowRunState::Validated;
    validated.revision = 1;
    validated.updated_at_unix_ms = 101;
    WorkflowRequestRecord {
        request_id: "wreq_start".into(),
        operation: "start".into(),
        payload: serde_json::json!({"request_id": "wreq_start", "spec": planned.spec}),
        response: serde_json::json!({"summary": WorkflowRunSummary::from(&validated)}),
    }
}

pub fn request() -> WorkflowRequestRecord {
    let run = snapshot();
    WorkflowRequestRecord {
        request_id: "wreq_get".into(),
        operation: "get".into(),
        payload: serde_json::json!({"request_id": "wreq_get", "run_id": "wrun_test"}),
        response: serde_json::to_value(WorkflowGetResponse { run: Some(run) }).unwrap(),
    }
}

pub fn write_init(path: &std::path::Path) {
    let journal = serde_json::json!({
        "kind": "init",
        "version": 1,
        "snapshot": snapshot(),
    });
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{}\n", journal)).unwrap();
}
