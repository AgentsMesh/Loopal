use std::sync::Arc;

use async_trait::async_trait;
use loopal_protocol::{AgentEventPayload, PermissionIntentRequest};
use loopal_provider_api::ContentBlock;
use loopal_runtime::AgentMode;
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::agent_loop::StreamingToolHandle;
use loopal_runtime::frontend::{AgentFrontend, EventEmitter};
use loopal_tool_api::{PermissionDecision, PermissionMode};

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

struct FailRollbackModeEvent(Arc<dyn AgentFrontend>);

#[async_trait]
impl AgentFrontend for FailRollbackModeEvent {
    async fn emit(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        self.0.emit(payload).await
    }

    async fn emit_in_turn(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        if matches!(&payload, AgentEventPayload::ModeChanged { mode } if mode == "act") {
            return Err(loopal_error::LoopalError::Other(
                "rollback mode event unavailable".into(),
            ));
        }
        self.0.emit_in_turn(payload).await
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        self.0.recv_input().await
    }

    async fn try_recv_input(&self) -> Result<AgentInput, tokio::sync::mpsc::error::TryRecvError> {
        self.0.try_recv_input().await
    }

    async fn request_permission(&self, request: &PermissionIntentRequest) -> PermissionDecision {
        self.0.request_permission(request).await
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        self.0.event_emitter()
    }
}

#[tokio::test]
async fn directory_failure_rolls_back_even_when_the_rollback_event_fails() {
    let (mut runner, _events, _mailbox, _control, permission) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    runner.params.deps.frontend =
        Arc::new(FailRollbackModeEvent(runner.params.deps.frontend.clone()));
    std::fs::create_dir_all(&runner.params.session.cwd).unwrap();
    std::fs::write(
        std::path::Path::new(&runner.params.session.cwd).join(".loopal"),
        "blocks plan directory",
    )
    .unwrap();
    permission.send(true).await.unwrap();
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

    assert_eq!(runner.params.config.mode, AgentMode::Act);
    assert!(runner.params.config.plan_state.is_none());
    assert!(matches!(
        &runner.turns.view().messages()[0].content[0],
        ContentBlock::ToolResult { content, is_error: true, .. }
            if content.contains("Plan mode was not entered")
    ));
}
