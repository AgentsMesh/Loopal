use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_agent::workflow_control::WorkflowStartControlError;
use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_protocol::{Envelope, MessageSource, WorkflowStartRequest, WorkflowStartResponse};
use loopal_tool_api::{OneShotChatError, OneShotChatService};
use tokio::sync::Mutex;

use super::ProactiveWorkflowInputHandler;

#[path = "workflow_input_control_test_support.rs"]
mod control;
pub(crate) use control::ControlStub;

struct ChatStub {
    replies: Mutex<VecDeque<Result<String, OneShotChatError>>>,
    delay: Option<Duration>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl OneShotChatService for ChatStub {
    async fn one_shot_chat(
        &self,
        _model: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        self.replies
            .lock()
            .await
            .pop_front()
            .expect("unexpected planner call")
    }
}

pub(super) fn workflow_plan() -> String {
    let spec = loopal_protocol::WorkflowSpec {
        version: loopal_protocol::WORKFLOW_SPEC_V1,
        run_goal: "compare independent implementations".into(),
        nodes: vec![loopal_protocol::WorkflowAgentNode {
            id: loopal_protocol::WorkflowNodeId::new("worker"),
            dependencies: Vec::new(),
            task: "inspect one implementation".into(),
            worker_profile: loopal_protocol::WorkflowWorkerProfileRef::new("default"),
        }],
        limits: loopal_protocol::WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 1_024,
        },
        output_node: loopal_protocol::WorkflowNodeId::new("worker"),
        output_contract: loopal_protocol::WorkflowOutputContract::Text { max_bytes: 1_024 },
    };
    serde_json::to_string(&loopal_protocol::WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: loopal_protocol::WorkflowExecution::Workflow { spec },
    })
    .unwrap()
}

pub(super) fn request_for(env: &Envelope, plan: &str) -> WorkflowStartRequest {
    let decision: loopal_protocol::WorkflowPlanDecision = serde_json::from_str(plan).unwrap();
    let loopal_protocol::WorkflowExecution::Workflow { spec } = decision.execution else {
        panic!("fixture must be a workflow decision");
    };
    WorkflowStartRequest {
        request_id: loopal_protocol::WorkflowRequestId::new(format!("human_{}", env.id.simple())),
        spec,
    }
}

pub(super) fn response(request: &WorkflowStartRequest) -> WorkflowStartResponse {
    let run = loopal_protocol::WorkflowRunSnapshot::planned(
        loopal_protocol::WorkflowRunId::new("wrun_test"),
        loopal_protocol::QualifiedAddress::local("main"),
        request.spec.clone(),
        1,
    );
    WorkflowStartResponse {
        summary: (&run).into(),
    }
}

pub(super) fn handler(
    chat_replies: Vec<Result<String, OneShotChatError>>,
    start_results: Vec<Result<WorkflowStartResponse, WorkflowStartControlError>>,
) -> (Arc<ProactiveWorkflowInputHandler>, Arc<ControlStub>) {
    handler_with_delay(chat_replies, start_results, None)
}

pub(super) fn handler_with_delay(
    chat_replies: Vec<Result<String, OneShotChatError>>,
    start_results: Vec<Result<WorkflowStartResponse, WorkflowStartControlError>>,
    delay: Option<Duration>,
) -> (Arc<ProactiveWorkflowInputHandler>, Arc<ControlStub>) {
    let settings = WorkflowSettings {
        execution_enabled: true,
        policy: OrchestrationPolicy::Proactive,
        ..Default::default()
    };
    let control = Arc::new(ControlStub {
        results: Mutex::new(VecDeque::from(start_results)),
        requests: Mutex::new(Vec::new()),
        lookups: Mutex::new(VecDeque::new()),
        lookup_requests: Mutex::new(Vec::new()),
    });
    let planner = loopal_agent::ProactiveWorkflowPlanner::new(
        settings,
        Arc::new(ChatStub {
            replies: Mutex::new(VecDeque::from(chat_replies)),
            delay,
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }),
        "test-model",
    );
    (
        Arc::new(ProactiveWorkflowInputHandler::new(planner, control.clone())),
        control,
    )
}

pub(super) fn planner_handler(
    settings: WorkflowSettings,
    calls: Arc<std::sync::atomic::AtomicUsize>,
) -> (ProactiveWorkflowInputHandler, Arc<ControlStub>) {
    let control = Arc::new(ControlStub {
        results: Mutex::new(VecDeque::new()),
        requests: Mutex::new(Vec::new()),
        lookups: Mutex::new(VecDeque::new()),
        lookup_requests: Mutex::new(Vec::new()),
    });
    let planner = loopal_agent::ProactiveWorkflowPlanner::new(
        settings,
        Arc::new(ChatStub {
            replies: Mutex::new(VecDeque::new()),
            delay: None,
            calls,
        }),
        "test-model",
    );
    (
        ProactiveWorkflowInputHandler::new(planner, control.clone()),
        control,
    )
}

pub(super) fn envelope() -> Envelope {
    Envelope::new(
        MessageSource::Human,
        "main",
        "Inspect the repository and ask several agents to independently cross-check the result",
    )
}
