use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_protocol::{
    ControlCommand, Envelope, PermissionIntent, PermissionIntentRequest, PermissionReceipt,
    WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};
use loopal_runtime::agent_loop::StreamingToolHandle;
use loopal_runtime::frontend::{
    PermissionHandler, PermissionOutcome, UnifiedFrontend, UnsupportedQuestionHandler,
};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle};
use loopal_test_support::TestFixture;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

use super::{in_turn, make_test_budget, make_turn_ctx};

struct Capture(Arc<Mutex<Option<PermissionIntentRequest>>>);

#[async_trait]
impl PermissionHandler for Capture {
    async fn decide(&self, request: &PermissionIntentRequest) -> PermissionOutcome {
        *self.0.lock().unwrap() = Some(request.clone());
        PermissionOutcome::deny("captured")
    }
}

struct MismatchedReceipt;

#[async_trait]
impl PermissionHandler for MismatchedReceipt {
    async fn decide(&self, request: &PermissionIntentRequest) -> PermissionOutcome {
        let other = PermissionIntentRequest::create(
            "other-call",
            request.tool_name.clone(),
            request.action_input.clone(),
            request.display_input.clone(),
            request.tool_schema.clone(),
            request.intent_seed.workflow().cloned(),
        )
        .unwrap();
        let intent = PermissionIntent::bind(other.intent_seed, 7, 3, "wrong-intent").unwrap();
        PermissionOutcome {
            decision: loopal_tool_api::PermissionDecision::Allow,
            reason: "wrong receipt".into(),
            duration_ms: 0,
            receipt: Some(PermissionReceipt::issue_for_intent(&intent, "audit-wrong").unwrap()),
        }
    }
}

fn causation() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_test"),
        node_id: WorkflowNodeId::new("node_test"),
        attempt_id: WorkflowAttemptId::new("watt_test"),
    }
}

fn runner(
    fixture: &TestFixture,
    workflow: WorkflowPermissionCausation,
    captured: Arc<Mutex<Option<PermissionIntentRequest>>>,
) -> loopal_runtime::agent_loop::AgentLoopRunner {
    runner_with_handler(fixture, workflow, Box::new(Capture(captured)))
}

fn runner_with_handler(
    fixture: &TestFixture,
    workflow: WorkflowPermissionCausation,
    permission: Box<dyn PermissionHandler>,
) -> loopal_runtime::agent_loop::AgentLoopRunner {
    let (event_tx, mut event_rx) = mpsc::channel(8);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let (_mailbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(8);
    let (_control_tx, control_rx) = mpsc::channel::<ControlCommand>(8);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        permission,
        Box::new(UnsupportedQuestionHandler),
    ));
    let kernel = Arc::new(loopal_kernel::Kernel::new(Default::default()).unwrap());
    let params = AgentLoopParamsBuilder::new(
        AgentConfig {
            permission_mode: PermissionMode::AskAnyWrite,
            ..Default::default()
        },
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
            protected_effect_audit: super::noop_protected_effect_audit(),
        },
        fixture.test_session("workflow-permission"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .workflow_permission_causation_opt(Some(workflow))
    .build();
    loopal_runtime::agent_loop::AgentLoopRunner::new(params)
}

#[tokio::test]
async fn runtime_permission_request_carries_workflow_causation() {
    let workflow = causation();
    let captured = Arc::new(Mutex::new(None));
    let fixture = TestFixture::new();
    let runner = runner(&fixture, workflow.clone(), captured.clone());

    let decision = runner
        .check_permission("call", "Write", &serde_json::json!({}))
        .await
        .unwrap();

    assert_eq!(decision, loopal_tool_api::PermissionDecision::Deny);
    let request = captured.lock().unwrap().take().unwrap();
    assert_eq!(request.intent_seed.workflow(), Some(&workflow));
}

#[tokio::test]
async fn enter_plan_permission_carries_workflow_causation() {
    let workflow = causation();
    let captured = Arc::new(Mutex::new(None));
    let fixture = TestFixture::new();
    let mut runner = runner(&fixture, workflow.clone(), captured.clone());
    let mut turn = make_turn_ctx();

    in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "enter".into(),
            "EnterPlanMode".into(),
            serde_json::json!({}),
        )],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let request = captured.lock().unwrap().take().unwrap();
    assert_eq!(request.intent_seed.workflow(), Some(&workflow));
}

#[tokio::test]
async fn workflow_tool_rejects_receipt_bound_to_another_action() {
    let fixture = TestFixture::new();
    let mut runner = runner_with_handler(&fixture, causation(), Box::new(MismatchedReceipt));
    let mut turn = make_turn_ctx();

    let error = in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "write".into(),
            "Write".into(),
            serde_json::json!({"file_path": "/tmp/loopal-wrong-receipt", "content": "x"}),
        )],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("permission receipt binding mismatch")
    );
}
