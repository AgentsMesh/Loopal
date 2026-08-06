use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_protocol::{AgentEvent, AgentEventPayload, Question, UserQuestionResponse};
use loopal_provider_api::ContentBlock;
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::agent_loop::{PlanModeState, StreamingToolHandle};
use loopal_runtime::frontend::{AgentFrontend, EventEmitter};
use loopal_runtime::{AgentMode, PlanApproval, PlanApprovalCancellationReason};
use loopal_tool_api::PermissionDecision;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use tokio::sync::Notify;

use super::{in_turn, make_runner, make_turn_ctx};

struct PlanFrontend {
    inner: Arc<dyn AgentFrontend>,
    cancellation: Option<PlanApprovalCancellationReason>,
    started: Arc<Notify>,
}

#[async_trait]
impl AgentFrontend for PlanFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        self.inner.emit(payload).await
    }

    async fn emit_in_turn(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        self.inner.emit_in_turn(payload).await
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        self.inner.recv_input().await
    }

    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision {
        self.inner.request_permission(id, name, input).await
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        self.inner.event_emitter()
    }

    async fn drain_pending(&self) -> Vec<AgentInput> {
        self.inner.drain_pending().await
    }

    async fn ask_user(&self, questions: Vec<Question>) -> UserQuestionResponse {
        self.inner.ask_user(questions).await
    }

    async fn request_plan_approval(&self, _plan: &str, _path: &str) -> PlanApproval {
        self.started.notify_one();
        match self.cancellation {
            Some(reason) => PlanApproval::Cancelled(reason),
            None => pending().await,
        }
    }

    fn try_emit(&self, payload: AgentEventPayload) -> bool {
        self.inner.try_emit(payload)
    }
}

fn prepare_runner(
    cancellation: Option<PlanApprovalCancellationReason>,
) -> (
    loopal_runtime::agent_loop::AgentLoopRunner,
    Arc<Notify>,
    tokio::sync::mpsc::Receiver<AgentEvent>,
) {
    let (mut runner, events) = make_runner();
    let started = Arc::new(Notify::new());
    runner.params.deps.frontend = Arc::new(PlanFrontend {
        inner: runner.params.deps.frontend.clone(),
        cancellation,
        started: started.clone(),
    });
    runner.params.config.mode = AgentMode::Plan;
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: runner.params.config.permission_mode,
        tool_filter: Default::default(),
    });
    std::fs::create_dir_all(runner.plan_file.path().parent().unwrap()).unwrap();
    std::fs::write(runner.plan_file.path(), "# Plan").unwrap();
    (runner, started, events)
}

fn exit_plan_tool() -> Vec<(String, String, serde_json::Value)> {
    vec![("exit".into(), "ExitPlanMode".into(), serde_json::json!({}))]
}

fn assert_error_result(runner: &loopal_runtime::agent_loop::AgentLoopRunner, expected: &str) {
    let message = &runner.turns.view().messages()[0];
    match &message.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error);
            assert!(content.contains(expected), "unexpected result: {content}");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

fn assert_event_metadata(
    events: &mut tokio::sync::mpsc::Receiver<AgentEvent>,
    expected: Option<ToolResultMetadata>,
) {
    while let Ok(event) = events.try_recv() {
        if let AgentEventPayload::ToolResult { id, metadata, .. } = event.payload
            && id == "exit"
        {
            assert_eq!(metadata, expected);
            return;
        }
    }
    panic!("missing ExitPlanMode ToolResult event");
}

#[tokio::test]
async fn unavailable_plan_approval_ends_turn_without_retrying() {
    let (mut runner, _started, mut events) =
        prepare_runner(Some(PlanApprovalCancellationReason::Unavailable));
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        exit_plan_tool(),
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert!(turn_ctx.turn_end_after_tools_signaled());
    assert_error_result(&runner, "Plan approval is unavailable");
    assert_event_metadata(&mut events, None);
}

#[tokio::test]
async fn interrupt_cancels_blocked_exit_plan_approval() {
    let (mut runner, started, mut events) = prepare_runner(None);
    let signal = runner.interrupt.clone();
    let interrupt_tx = runner.interrupt_tx.clone();
    tokio::spawn(async move {
        started.notified().await;
        signal.signal();
        interrupt_tx.send_modify(|generation| *generation += 1);
    });
    let mut turn_ctx = loopal_runtime::agent_loop::TurnContext::new(
        0,
        loopal_runtime::agent_loop::cancel::TurnCancel::new(
            runner.interrupt.clone(),
            runner.interrupt_tx.clone(),
        ),
    );

    tokio::time::timeout(
        Duration::from_secs(1),
        in_turn(runner.execute_tools(
            &mut turn_ctx,
            exit_plan_tool(),
            StreamingToolHandle::empty(),
        )),
    )
    .await
    .expect("ExitPlanMode must stop waiting after interrupt")
    .unwrap();
    assert!(turn_ctx.cancel.is_cancelled());
    assert!(turn_ctx.turn_end_after_tools_signaled());
    assert_error_result(&runner, "Interrupted by user");
    assert_event_metadata(
        &mut events,
        Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
    );
}
