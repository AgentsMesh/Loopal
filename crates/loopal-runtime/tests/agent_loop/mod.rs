use std::sync::Arc;

use loopal_config::Settings;
use loopal_context::{ContextBudget, ContextStore};
use loopal_kernel::Kernel;
use loopal_protocol::AgentEvent;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_runtime::agent_loop::{AgentLoopRunner, cancel::TurnCancel};
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler, TuiPermissionHandler};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend};
use loopal_test_support::TestFixture;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

/// Create a no-op TurnCancel for tests (never cancelled).
pub fn make_cancel() -> TurnCancel {
    TurnCancel::new(
        Default::default(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    )
}

pub fn make_test_budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_384,
        safety_margin: 10_000,
        message_budget: 173_616,
    }
}

mod auto_continue_edge_test;
mod auto_continue_test;
mod drain_pending_test;
mod input_edge_test;
mod input_image_test;
mod input_test;
mod integration_test;
mod llm_test;
pub mod mock_provider;
pub use mock_provider::make_runner_with_mock_provider;
mod cancel_test;
mod context_budget_test;
mod permission_test_ext;
mod preflight_test;
mod record_message_test;
mod retry_cancel_test;
mod run_test;
mod tools_completion_test;
mod tools_test;
mod turn_completion_test;
mod turn_test;

/// Minimal runner with no provider — for testing pure AgentLoopRunner methods.
pub fn make_runner() -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let params = AgentLoopParams {
        config: AgentConfig::default(),
        deps: AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("test-minimal"),
        store: ContextStore::new(make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
    };
    (AgentLoopRunner::new(params), event_rx)
}

/// Runner with all channels exposed — for testing permission and input flows.
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
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx.clone(),
        mailbox_rx,
        control_rx,
        None,
        Box::new(TuiPermissionHandler::new(event_tx, permission_rx)),
        Box::new(AutoCancelQuestionHandler),
    ));
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let params = AgentLoopParams {
        config: AgentConfig {
            permission_mode: PermissionMode::Supervised,
            ..Default::default()
        },
        deps: AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("test-channels"),
        store: ContextStore::new(make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
    };
    (
        AgentLoopRunner::new(params),
        event_rx,
        mbox_tx,
        ctrl_tx,
        perm_tx,
    )
}
