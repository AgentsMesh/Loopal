use std::sync::Arc;

use async_trait::async_trait;
use loopal_protocol::{AgentEventPayload, PermissionIntentRequest};
use loopal_provider_api::ContentBlock;
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::agent_loop::{PlanModeState, StreamingToolHandle};
use loopal_runtime::frontend::{AgentFrontend, EventEmitter};
use loopal_runtime::{AgentMode, PlanApproval};
use loopal_tool_api::{PermissionDecision, PermissionMode};

use super::{in_turn, make_runner, make_turn_ctx};

struct ApprovalFrontend {
    inner: Arc<dyn AgentFrontend>,
    approval: PlanApproval,
}

#[async_trait]
impl AgentFrontend for ApprovalFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        self.inner.emit(payload).await
    }

    async fn emit_in_turn(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        self.inner.emit_in_turn(payload).await
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        self.inner.recv_input().await
    }

    async fn try_recv_input(&self) -> Result<AgentInput, tokio::sync::mpsc::error::TryRecvError> {
        self.inner.try_recv_input().await
    }

    async fn request_permission(&self, request: &PermissionIntentRequest) -> PermissionDecision {
        self.inner.request_permission(request).await
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        self.inner.event_emitter()
    }

    async fn request_plan_approval(&self, _: &str, _: &str) -> PlanApproval {
        self.approval.clone()
    }
}

fn runner(approval: PlanApproval) -> loopal_runtime::agent_loop::AgentLoopRunner {
    let (mut runner, mut events) = make_runner();
    tokio::spawn(async move { while events.recv().await.is_some() {} });
    runner.params.deps.frontend = Arc::new(ApprovalFrontend {
        inner: runner.params.deps.frontend.clone(),
        approval,
    });
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: PermissionMode::AskAnyWrite,
        tool_filter: Default::default(),
    });
    runner.params.config.mode = AgentMode::Plan;
    std::fs::create_dir_all(runner.plan_file.path().parent().unwrap()).unwrap();
    std::fs::write(runner.plan_file.path(), "# Original").unwrap();
    runner
}

async fn exit(runner: &mut loopal_runtime::agent_loop::AgentLoopRunner) {
    let mut turn = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn,
        vec![("exit".into(), "ExitPlanMode".into(), serde_json::json!({}))],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();
}

fn result(runner: &loopal_runtime::agent_loop::AgentLoopRunner) -> (&str, bool) {
    match &runner.turns.view().messages()[0].content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => (content, *is_error),
        other => panic!("expected tool result, got {other:?}"),
    }
}

#[tokio::test]
async fn approval_with_edits_persists_and_restores_snapshot() {
    let mut runner = runner(PlanApproval::ApproveWithEdits("# Edited".into()));
    exit(&mut runner).await;

    assert_eq!(
        std::fs::read_to_string(runner.plan_file.path()).unwrap(),
        "# Edited"
    );
    assert_eq!(runner.params.config.mode, AgentMode::Act);
    assert_eq!(
        runner.params.config.permission_mode,
        PermissionMode::AskAnyWrite
    );
    assert!(runner.params.config.plan_state.is_none());
    let (content, is_error) = result(&runner);
    assert!(!is_error);
    assert!(content.contains("# Edited"));
}

#[tokio::test]
async fn approval_without_snapshot_defaults_to_act() {
    let mut runner = runner(PlanApproval::Approve);
    runner.params.config.plan_state = None;
    exit(&mut runner).await;

    assert_eq!(runner.params.config.mode, AgentMode::Act);
    let (content, is_error) = result(&runner);
    assert!(!is_error);
    assert!(content.contains("# Original"));
}

#[tokio::test]
async fn approval_restores_previous_plan_mode() {
    let mut runner = runner(PlanApproval::Approve);
    runner
        .params
        .config
        .plan_state
        .as_mut()
        .unwrap()
        .previous_mode = AgentMode::Plan;
    exit(&mut runner).await;

    assert_eq!(runner.params.config.mode, AgentMode::Plan);
}

#[tokio::test]
async fn rejection_preserves_plan_state_for_revision() {
    let mut runner = runner(PlanApproval::Reject);
    exit(&mut runner).await;

    assert_eq!(runner.params.config.mode, AgentMode::Plan);
    assert!(runner.params.config.plan_state.is_some());
    assert_eq!(
        std::fs::read_to_string(runner.plan_file.path()).unwrap(),
        "# Original"
    );
    let (content, is_error) = result(&runner);
    assert!(!is_error);
    assert!(content.contains("rejected the plan"));
}
