use std::sync::Arc;

use chrono::Utc;
use loopal_config::Settings;
use loopal_context::{ContextBudget, ContextPipeline, ContextStore};
use loopal_kernel::Kernel;
use loopal_protocol::AgentEvent;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_runtime::agent_loop::{AgentLoopRunner, cancel::TurnCancel};
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler, TuiPermissionHandler};
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend};
use loopal_storage::Session;
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
mod permission_test_ext;
mod preflight_test;
mod record_message_test;
mod retry_cancel_test;
mod run_test;
mod tools_completion_test;
mod tools_test;
mod turn_completion_test;
mod turn_test;

/// Create an AgentLoopRunner with minimal/mock parameters for testing
/// pure methods (prepare_chat_params, record_assistant_message, emit).
pub fn make_runner() -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
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

    let kernel = Arc::new(
        Kernel::new(Settings::default()).expect("Kernel::new with defaults should succeed"),
    );
    let session = Session {
        id: "test-session-001".to_string(),
        title: "Test Session".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!("loopal_test_{}", std::process::id()));
    let session_manager = SessionManager::with_base_dir(tmp_dir);

    let params = AgentLoopParams {
        kernel,
        session,
        store: ContextStore::new(make_test_budget()),
        model: "claude-sonnet-4-20250514".to_string(),
        compact_model: None,
        system_prompt: "You are a helpful assistant.".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Bypass,
        max_turns: 10,
        frontend,
        session_manager,
        context_pipeline: ContextPipeline::new(),
        tool_filter: None,
        shared: None,
        interactive: true,
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: Default::default(),
        interrupt_tx: std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
        memory_channel: None,
    };

    (AgentLoopRunner::new(params), event_rx)
}

/// Create a runner with mailbox, control, and permission channels exposed
/// for driving async methods like wait_for_input and check_permission.
pub fn make_runner_with_channels() -> (
    AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<Envelope>,
    mpsc::Sender<ControlCommand>,
    mpsc::Sender<bool>,
) {
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

    let kernel = Arc::new(
        Kernel::new(Settings::default()).expect("Kernel::new with defaults should succeed"),
    );
    let session = Session {
        id: "test-chan-001".to_string(),
        title: "Test".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!("loopal_test_chan_{}", std::process::id()));
    let session_manager = SessionManager::with_base_dir(tmp_dir);

    let params = AgentLoopParams {
        kernel,
        session,
        store: ContextStore::new(make_test_budget()),
        model: "claude-sonnet-4-20250514".to_string(),
        compact_model: None,
        system_prompt: "Test prompt.".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Supervised,
        max_turns: 10,
        frontend,
        session_manager,
        context_pipeline: ContextPipeline::new(),
        tool_filter: None,
        shared: None,
        interactive: true,
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: Default::default(),
        interrupt_tx: std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
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
