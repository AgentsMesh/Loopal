use std::sync::Arc;

use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{ManualPermissionHandler, UnsupportedQuestionHandler};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle, UnifiedFrontend,
};
use loopal_test_support::TestFixture;
use tokio::sync::mpsc;

use super::make_test_budget;

pub fn make_runner_with_kernel(
    kernel: Arc<Kernel>,
) -> (
    AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<bool>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, permission_rx) = mpsc::channel(1);
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
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
            protected_effect_audit: super::noop_protected_effect_audit(),
        },
        fixture.test_session("test-custom-kernel"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .build();
    let mut runner = AgentLoopRunner::new(params);
    runner.start_turn_record(loopal_turn::TurnTrigger::Resume);
    (runner, event_rx, permission_tx)
}
