use std::sync::Arc;

use async_trait::async_trait;
use loopal_protocol::{AgentEventPayload, PermissionIntentRequest};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::agent_loop::{PlanModeState, StreamingToolHandle};
use loopal_runtime::frontend::{AgentFrontend, EventEmitter};
use loopal_runtime::{AgentMode, PlanApproval, PlanApprovalCancellationReason};
use loopal_tool_api::{
    PermissionDecision, PermissionLevel, PermissionMode, Tool, ToolContext, ToolResult,
};

use super::{in_turn, make_runner, make_turn_ctx};

struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "Agent"
    }

    fn description(&self) -> &str {
        "test agent"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        _: serde_json::Value,
        _: &ToolContext,
    ) -> Result<ToolResult, loopal_error::LoopalError> {
        Ok(ToolResult::success("unused"))
    }
}

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
    runner.params.config.mode = AgentMode::Plan;
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: PermissionMode::AskAnyWrite,
        tool_filter: Default::default(),
    });
    std::fs::create_dir_all(runner.plan_file.path().parent().unwrap()).unwrap();
    std::fs::write(runner.plan_file.path(), "# Plan").unwrap();
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

fn result(runner: &loopal_runtime::agent_loop::AgentLoopRunner) -> &str {
    let loopal_provider_api::ContentBlock::ToolResult { content, .. } =
        &runner.turns.view().messages()[0].content[0]
    else {
        panic!("expected tool result")
    };
    content
}

#[tokio::test]
async fn approval_includes_agent_parallelization_hint() {
    let mut runner = runner(PlanApproval::Approve);
    runner.params.deps.kernel.register_tool(Box::new(AgentTool));
    exit(&mut runner).await;
    assert!(result(&runner).contains("Agent tool to parallelize"));
}

#[tokio::test]
async fn non_interrupt_cancellations_have_specific_messages() {
    for (reason, expected) in [
        (PlanApprovalCancellationReason::TimedOut, "timed out"),
        (PlanApprovalCancellationReason::Superseded, "superseded"),
        (
            PlanApprovalCancellationReason::Transport,
            "connection was lost",
        ),
    ] {
        let mut runner = runner(PlanApproval::Cancelled(reason));
        exit(&mut runner).await;
        assert!(result(&runner).contains(expected));
    }
}
