use async_trait::async_trait;
use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowRunId, WorkflowStartRequest, WorkflowStartResponse, WorkflowWaitRequest,
    WorkflowWaitResponse,
};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use std::sync::{Arc, Mutex};

use super::{cancel, get, schema, start, wait};
use crate::workflow_control::{WorkflowControlClient, WorkflowStartControlError};

struct RecordingClient {
    calls: Mutex<Vec<&'static str>>,
}

#[async_trait]
impl WorkflowControlClient for RecordingClient {
    async fn start(
        &self,
        _: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        self.calls.lock().unwrap().push("start");
        Err(WorkflowStartControlError::Rejected("start rejected".into()))
    }

    async fn get(&self, request: WorkflowGetRequest) -> Result<WorkflowGetResponse, String> {
        self.calls.lock().unwrap().push("get");
        Ok(WorkflowGetResponse {
            run: (request.run_id == WorkflowRunId::new("wrun_known")).then_some(run()),
        })
    }

    async fn wait(&self, _: WorkflowWaitRequest) -> Result<WorkflowWaitResponse, String> {
        self.calls.lock().unwrap().push("wait");
        Err("wait failed".into())
    }

    async fn cancel(&self, _: WorkflowCancelRequest) -> Result<WorkflowCancelResponse, String> {
        self.calls.lock().unwrap().push("cancel");
        Err("cancel failed".into())
    }
}

#[test]
fn tools_have_closed_secret_free_contracts() {
    let tools: [&dyn Tool; 4] = [
        &start::WorkflowStartTool,
        &get::WorkflowGetTool,
        &wait::WorkflowWaitTool,
        &cancel::WorkflowCancelTool,
    ];
    let names: Vec<_> = tools.iter().map(|tool| tool.name()).collect();
    assert_eq!(
        names,
        [
            "workflow_start",
            "workflow_get",
            "workflow_wait",
            "workflow_cancel"
        ]
    );
    for tool in tools {
        assert!(!tool.description().is_empty());
        assert!(tool.secret_eligible_params().is_empty());
        assert_eq!(tool.parameters_schema()["additionalProperties"], false);
    }
    assert_eq!(
        start::WorkflowStartTool.permission(),
        PermissionLevel::Write
    );
    assert_eq!(get::WorkflowGetTool.permission(), PermissionLevel::ReadOnly);
    assert_eq!(
        wait::WorkflowWaitTool.permission(),
        PermissionLevel::ReadOnly
    );
    assert_eq!(
        cancel::WorkflowCancelTool.permission(),
        PermissionLevel::Write
    );
}

#[test]
fn schemas_expose_protocol_limits() {
    let start = schema::start();
    assert_eq!(
        start["properties"]["spec"],
        loopal_protocol::workflow_spec_schema()
    );
    assert_eq!(
        start["properties"]["spec"]["properties"]["version"]["const"],
        1
    );
    assert_eq!(
        start["properties"]["spec"]["properties"]["nodes"]["maxItems"],
        512
    );
    assert_eq!(
        schema::wait()["properties"]["timeout_ms"]["maximum"],
        300_000
    );
}

#[tokio::test]
async fn get_uses_narrow_client_and_start_surfaces_rpc_error() {
    let client = Arc::new(RecordingClient {
        calls: Mutex::new(Vec::new()),
    });
    let ctx = context(Some(client.clone()));

    let get_result = get::WorkflowGetTool
        .execute(
            serde_json::json!({"request_id": "wreq_get", "run_id": "wrun_known"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!get_result.is_error);
    assert!(get_result.content.contains("wrun_known"));

    let start_result = start::WorkflowStartTool
        .execute(sample_start(), &ctx)
        .await
        .unwrap();
    assert!(start_result.is_error);
    let rejected: serde_json::Value = serde_json::from_str(&start_result.content).unwrap();
    assert_eq!(rejected["outcome"], "rejected");
    assert_eq!(rejected["fallback"], "direct");
    assert_eq!(rejected["message"], "start rejected");
    assert_eq!(*client.calls.lock().unwrap(), ["get", "start"]);
}

#[tokio::test]
async fn missing_capability_fails_closed() {
    let error = get::WorkflowGetTool
        .execute(
            serde_json::json!({"request_id": "wreq_get", "run_id": "wrun_known"}),
            &context(None),
        )
        .await
        .expect_err("missing workflow capability must be rejected");
    assert!(error.to_string().contains("workflow execution is disabled"));
}

fn context(client: Option<Arc<dyn WorkflowControlClient>>) -> ToolContext {
    let kernel = Arc::new(loopal_kernel::Kernel::new(loopal_config::Settings::default()).unwrap());
    let cwd = std::path::PathBuf::from(".");
    let backend = kernel.create_backend(&cwd, "workflow-tool-test");
    let (connection, _peer) = loopal_ipc::duplex_pair();
    let (hub_connection, _incoming) = loopal_ipc::Connection::new(connection).into_listening();
    let shared = Arc::new(crate::AgentShared {
        kernel,
        task_store: Arc::new(crate::TaskStore::with_session_storage(Arc::new(
            crate::InMemoryTaskStorage::new(),
        ))),
        hub_connection,
        cwd,
        depth: 0,
        agent_name: "main".into(),
        parent_event_tx: None,
        cancel_token: None,
        scheduler_handle: crate::shared::SchedulerHandle::new(
            Arc::new(loopal_scheduler::CronScheduler::new()),
            tokio_util::sync::CancellationToken::new(),
        ),
        message_snapshot: Arc::new(std::sync::RwLock::new(Vec::new())),
        goal_session: None,
        workflow_control: client,
    });
    ToolContext::new(backend, "workflow-tool-test").with_shared(Arc::new(shared))
}

fn sample_start() -> serde_json::Value {
    serde_json::json!({
        "request_id": "wreq_start",
        "spec": {
            "version": 1,
            "run_goal": "test",
            "nodes": [{"id": "node", "dependencies": [], "task": "work", "worker_profile": "default"}],
            "limits": {"max_nodes": 1, "max_parallel": 1, "max_attempts": 1, "run_deadline_ms": 1000, "attempt_timeout_ms": 500, "max_output_bytes": 1024},
            "output_node": "node",
            "output_contract": {"type": "text", "max_bytes": 1024}
        }
    })
}

fn run() -> loopal_protocol::WorkflowRunSnapshot {
    let request: WorkflowStartRequest = serde_json::from_value(sample_start()).unwrap();
    loopal_protocol::WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_known"),
        loopal_protocol::QualifiedAddress::local("main"),
        request.spec,
        1,
    )
}

#[path = "start_outcome_tests.rs"]
mod start_outcome_tests;
#[path = "tool_delegation_tests.rs"]
mod tool_delegation_tests;
