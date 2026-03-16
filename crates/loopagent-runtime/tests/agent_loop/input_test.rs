use loopagent_runtime::AgentMode;
use loopagent_types::command::UserCommand;
use loopagent_types::event::AgentEvent;
use loopagent_types::message::Message;

use super::{make_runner, make_runner_with_channels};

#[test]
fn test_agent_loop_runner_construction() {
    let (runner, _rx) = make_runner();
    assert_eq!(runner.turn_count, 0);
    assert_eq!(runner.total_input_tokens, 0);
    assert_eq!(runner.total_output_tokens, 0);
    assert_eq!(runner.params.model, "claude-sonnet-4-20250514");
    assert_eq!(runner.params.max_turns, 10);
}

#[test]
fn test_tool_ctx_matches_session() {
    let (runner, _rx) = make_runner();
    assert_eq!(
        runner.tool_ctx.cwd,
        std::path::PathBuf::from("/tmp")
    );
    assert_eq!(runner.tool_ctx.session_id, "test-session-001");
}

#[test]
fn test_model_info_defaults_for_unknown_model() {
    use std::sync::Arc;
    use chrono::Utc;
    use loopagent_context::ContextPipeline;
    use loopagent_kernel::Kernel;
    use loopagent_runtime::{AgentLoopParams, SessionManager};
    use loopagent_storage::Session;
    use loopagent_types::config::Settings;
    use loopagent_types::permission::PermissionMode;
    use loopagent_runtime::agent_loop::AgentLoopRunner;
    use tokio::sync::mpsc;

    let (event_tx, _event_rx) = mpsc::channel(16);
    let (_input_tx, input_rx) = mpsc::channel(16);
    let (_perm_tx, permission_rx) = mpsc::channel(16);

    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = Session {
        id: "test".to_string(),
        title: "".to_string(),
        model: "unknown-model-xyz".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!(
        "loopagent_test_unknown_{}",
        std::process::id()
    ));
    let session_manager = SessionManager::with_base_dir(tmp_dir);
    let context_pipeline = ContextPipeline::new();

    let params = AgentLoopParams {
        kernel,
        session,
        messages: Vec::new(),
        model: "unknown-model-xyz".to_string(),
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Default,
        max_turns: 5,
        event_tx,
        input_rx,
        permission_rx,
        session_manager,
        context_pipeline,
    };

    let runner = AgentLoopRunner::new(params);
    // Unknown model should fall back to defaults
    assert_eq!(runner.max_context_tokens, 200_000);
}

#[tokio::test]
async fn test_wait_for_input_message() {
    let (mut runner, mut event_rx, input_tx, _perm_tx) = make_runner_with_channels();

    input_tx
        .send(UserCommand::Message("Hello agent".to_string()))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());
    assert_eq!(runner.params.messages.len(), 1);
    assert_eq!(runner.params.messages[0].text_content(), "Hello agent");

    // Should have emitted AwaitingInput
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(event, AgentEvent::AwaitingInput));
}

#[tokio::test]
async fn test_wait_for_input_mode_switch() {
    let (mut runner, mut event_rx, input_tx, _perm_tx) = make_runner_with_channels();

    input_tx
        .send(UserCommand::ModeSwitch(loopagent_types::command::AgentMode::Plan))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());
    assert_eq!(runner.params.mode, AgentMode::Plan);

    // Should have emitted AwaitingInput, then ModeChanged
    let e1 = event_rx.recv().await.unwrap();
    assert!(matches!(e1, AgentEvent::AwaitingInput));
    let e2 = event_rx.recv().await.unwrap();
    assert!(matches!(e2, AgentEvent::ModeChanged { mode } if mode == "plan"));
}

#[tokio::test]
async fn test_wait_for_input_channel_closed() {
    let (mut runner, _event_rx, input_tx, _perm_tx) = make_runner_with_channels();
    drop(input_tx); // close input channel

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_execute_middleware_empty_pipeline() {
    let (mut runner, _event_rx, _input_tx, _perm_tx) = make_runner_with_channels();
    runner.params.messages.push(Message::user("test"));

    let should_continue = runner.execute_middleware().await.unwrap();
    assert!(should_continue);
    // Messages should be preserved
    assert_eq!(runner.params.messages.len(), 1);
}

#[tokio::test]
async fn test_emit_with_open_channel() {
    let (runner, mut rx) = make_runner();

    runner
        .emit(AgentEvent::Started)
        .await
        .expect("emit to open channel should succeed");

    let event = rx.recv().await.expect("should receive event");
    assert!(matches!(event, AgentEvent::Started));
}

#[tokio::test]
async fn test_emit_with_closed_channel() {
    let (runner, rx) = make_runner();
    drop(rx); // close the receiver

    let result = runner.emit(AgentEvent::Started).await;
    assert!(result.is_err(), "emit to closed channel should fail");
}

#[tokio::test]
async fn test_emit_multiple_events() {
    let (runner, mut rx) = make_runner();

    runner.emit(AgentEvent::Started).await.unwrap();
    runner
        .emit(AgentEvent::Stream {
            text: "hello".to_string(),
        })
        .await
        .unwrap();
    runner.emit(AgentEvent::Finished).await.unwrap();

    assert!(matches!(rx.recv().await.unwrap(), AgentEvent::Started));
    assert!(matches!(rx.recv().await.unwrap(), AgentEvent::Stream { text } if text == "hello"));
    assert!(matches!(rx.recv().await.unwrap(), AgentEvent::Finished));
}
