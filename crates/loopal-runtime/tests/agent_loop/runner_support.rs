use std::sync::Arc;

use loopal_config::Settings;
use loopal_context::ContextBudget;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_runtime::agent_loop::{AgentLoopRunner, cancel::TurnCancel};
use loopal_runtime::frontend::{
    DenyAllHandler, ManualPermissionHandler, UnsupportedQuestionHandler,
};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle, UnifiedFrontend,
};
use loopal_test_support::TestFixture;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

pub fn make_cancel() -> TurnCancel {
    TurnCancel::new(
        Default::default(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    )
}

pub fn make_turn_ctx() -> loopal_runtime::agent_loop::TurnContext {
    loopal_runtime::agent_loop::TurnContext::new(0, make_cancel())
}

pub async fn in_turn<F, R>(future: F) -> R
where
    F: std::future::Future<Output = R>,
{
    loopal_protocol::event_id::scope_turn(1, future).await
}

pub fn make_test_budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_384,
        safety_margin: 10_000,
        message_budget: 173_616,
        max_output_tokens: 64_000,
    }
}

pub fn make_runner() -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(
        UnifiedFrontend::new(
            None,
            event_tx,
            mailbox_rx,
            control_rx,
            None,
            Box::new(DenyAllHandler),
            Box::new(UnsupportedQuestionHandler),
        )
        .with_plan_approval(loopal_runtime::PlanApproval::Approve),
    );
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
        },
        fixture.test_session("test-minimal"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .build();
    let mut runner = AgentLoopRunner::new(params);
    runner.start_turn_record(loopal_turn::TurnTrigger::Resume);
    (runner, event_rx)
}

pub fn make_runner_with_channels() -> (
    AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<Envelope>,
    mpsc::Sender<ControlCommand>,
    mpsc::Sender<bool>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(16);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, permission_rx) = mpsc::channel::<bool>(16);
    let frontend = Arc::new(
        UnifiedFrontend::new(
            None,
            event_tx.clone(),
            mailbox_rx,
            control_rx,
            None,
            Box::new(ManualPermissionHandler::new(event_tx, permission_rx)),
            Box::new(UnsupportedQuestionHandler),
        )
        .with_plan_approval(loopal_runtime::PlanApproval::Approve),
    );
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
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
        },
        fixture.test_session("test-channels"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .build();
    let mut runner = AgentLoopRunner::new(params);
    runner.start_turn_record(loopal_turn::TurnTrigger::Resume);
    (runner, event_rx, mbox_tx, ctrl_tx, perm_tx)
}
