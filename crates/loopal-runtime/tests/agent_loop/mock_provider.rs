//! Shared `AgentLoopParams` construction for runtime unit tests.
//!
//! Provides the same API as the original factory functions but uses
//! sub-struct construction to eliminate 18-field boilerplate.

use std::sync::Arc;

use loopal_config::Settings;
use loopal_context::{ContextBudget, ContextStore};
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_provider_api::{Provider, StreamChunk};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend};
use loopal_test_support::TestFixture;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

pub use loopal_test_support::mock_provider::{MockProvider, MockStreamChunks, MultiCallProvider};

/// Build AgentLoopParams using sub-structs — eliminates the 18-field ceremony.
fn build_params(
    kernel: Arc<Kernel>,
    frontend: Arc<dyn loopal_runtime::AgentFrontend>,
    fixture: &TestFixture,
    messages: Vec<loopal_message::Message>,
    permission_mode: PermissionMode,
) -> AgentLoopParams {
    build_params_with_config(
        kernel,
        frontend,
        fixture,
        messages,
        AgentConfig {
            permission_mode,
            ..Default::default()
        },
    )
}

/// Build AgentLoopParams with a fully custom `AgentConfig`.
fn build_params_with_config(
    kernel: Arc<Kernel>,
    frontend: Arc<dyn loopal_runtime::AgentFrontend>,
    fixture: &TestFixture,
    messages: Vec<loopal_message::Message>,
    config: AgentConfig,
) -> AgentLoopParams {
    AgentLoopParams {
        config,
        deps: AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("rt-test"),
        store: ContextStore::from_messages(messages, make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
        scheduled_rx: None,
        auto_classifier: None,
        harness: loopal_config::HarnessConfig::default(),
        rewake_rx: None,
        message_snapshot: None,
    }
}

fn make_test_budget() -> ContextBudget {
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

pub fn make_runner_with_mock_provider(
    chunks: Vec<Result<StreamChunk, LoopalError>>,
) -> (
    AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<Envelope>,
    mpsc::Sender<ControlCommand>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(64);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_provider(Arc::new(MockProvider::new(chunks)) as Arc<dyn Provider>);
    let params = build_params(
        Arc::new(kernel),
        frontend,
        &fixture,
        vec![loopal_message::Message::user("hello")],
        PermissionMode::Bypass,
    );
    (AgentLoopRunner::new(params), event_rx, mbox_tx, ctrl_tx)
}

pub fn make_multi_runner(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    make_multi_runner_with_config(calls, AgentConfig::default())
}

/// Like `make_multi_runner` but accepts a custom `AgentConfig` (e.g. for
/// testing with `ThinkingConfig::Disabled`).
pub fn make_multi_runner_with_config(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
    config: AgentConfig,
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(64);
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
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_provider(Arc::new(MultiCallProvider::new(calls)) as Arc<dyn Provider>);
    let params = build_params_with_config(
        Arc::new(kernel),
        frontend,
        &fixture,
        vec![loopal_message::Message::user("go")],
        config,
    );
    (AgentLoopRunner::new(params), event_rx)
}

pub fn make_interactive_multi_runner(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
    setup: impl FnOnce(&mut Kernel),
) -> (
    AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<Envelope>,
    mpsc::Sender<ControlCommand>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(64);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_provider(Arc::new(MultiCallProvider::new(calls)) as Arc<dyn Provider>);
    setup(&mut kernel);
    let params = build_params(
        Arc::new(kernel),
        frontend,
        &fixture,
        vec![],
        PermissionMode::Bypass,
    );
    (AgentLoopRunner::new(params), event_rx, mbox_tx, ctrl_tx)
}
