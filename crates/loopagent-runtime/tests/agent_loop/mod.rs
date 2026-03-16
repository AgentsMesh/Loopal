use std::sync::Arc;

use chrono::Utc;
use loopagent_context::ContextPipeline;
use loopagent_kernel::Kernel;
use loopagent_runtime::agent_loop::AgentLoopRunner;
use loopagent_runtime::{AgentLoopParams, AgentMode, SessionManager};
use loopagent_storage::Session;
use loopagent_types::command::UserCommand;
use loopagent_types::config::Settings;
use loopagent_types::event::AgentEvent;
use loopagent_types::permission::PermissionMode;
use tokio::sync::mpsc;

mod input_test;
mod integration_test;
mod llm_test;
pub mod mock_provider;
mod permission_test_ext;
mod record_message_test;
mod run_test;
mod tools_test;

/// Create an AgentLoopRunner with minimal/mock parameters for testing
/// pure methods (prepare_chat_params, record_assistant_message, emit).
pub fn make_runner() -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_input_tx, input_rx) = mpsc::channel(16);
    let (_perm_tx, permission_rx) = mpsc::channel(16);

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

    let tmp_dir = std::env::temp_dir().join(format!(
        "loopagent_test_{}",
        std::process::id()
    ));
    let session_manager = SessionManager::with_base_dir(tmp_dir);
    let context_pipeline = ContextPipeline::new();

    let params = AgentLoopParams {
        kernel,
        session,
        messages: Vec::new(),
        model: "claude-sonnet-4-20250514".to_string(),
        system_prompt: "You are a helpful assistant.".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::BypassPermissions,
        max_turns: 10,
        event_tx,
        input_rx,
        permission_rx,
        session_manager,
        context_pipeline,
    };

    let runner = AgentLoopRunner::new(params);
    (runner, event_rx)
}

/// Create a runner that also returns the input and permission senders
/// for driving async methods like wait_for_input and check_permission.
pub fn make_runner_with_channels() -> (
    AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<UserCommand>,
    mpsc::Sender<bool>,
) {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (input_tx, input_rx) = mpsc::channel(16);
    let (perm_tx, permission_rx) = mpsc::channel(16);

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

    let tmp_dir = std::env::temp_dir().join(format!(
        "loopagent_test_chan_{}",
        std::process::id()
    ));
    let session_manager = SessionManager::with_base_dir(tmp_dir);
    let context_pipeline = ContextPipeline::new();

    let params = AgentLoopParams {
        kernel,
        session,
        messages: Vec::new(),
        model: "claude-sonnet-4-20250514".to_string(),
        system_prompt: "Test prompt.".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Default,
        max_turns: 10,
        event_tx,
        input_rx,
        permission_rx,
        session_manager,
        context_pipeline,
    };

    let runner = AgentLoopRunner::new(params);
    (runner, event_rx, input_tx, perm_tx)
}
