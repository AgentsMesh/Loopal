use std::sync::Arc;
use std::time::Duration;

use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;

use super::{make_runner, make_runner_with_channels};

#[test]
fn test_model_info_defaults_for_unknown_model() {
    use chrono::Utc;
    use loopal_config::Settings;
    use loopal_context::ContextPipeline;
    use loopal_kernel::Kernel;
    use loopal_runtime::agent_loop::AgentLoopRunner;
    use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
    use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend};
    use loopal_storage::Session;
    use loopal_tool_api::PermissionMode;
    use tokio::sync::mpsc;

    let (event_tx, _event_rx) = mpsc::channel(16);
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
    let session = Session {
        id: "test".to_string(),
        title: "".to_string(),
        model: "unknown-model-xyz".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!("loopal_test_unknown_{}", std::process::id()));
    let session_manager = SessionManager::with_base_dir(tmp_dir);
    let context_pipeline = ContextPipeline::new();

    let params = AgentLoopParams {
        kernel,
        session,
        messages: Vec::new(),
        model: "unknown-model-xyz".to_string(),
        compact_model: None,
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Supervised,
        max_turns: 5,
        frontend,
        session_manager,
        context_pipeline,
        tool_filter: None,
        shared: None,
        interactive: true,
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: Default::default(),
        interrupt_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
        memory_channel: None,
    };

    let runner = AgentLoopRunner::new(params);
    // Unknown model should fall back to defaults
    assert_eq!(runner.model_config.max_context_tokens, 200_000);
}

#[tokio::test]
async fn test_emit_multiple_events() {
    let (runner, mut rx) = make_runner();

    runner.emit(AgentEventPayload::Started).await.unwrap();
    runner
        .emit(AgentEventPayload::Stream {
            text: "hello".to_string(),
        })
        .await
        .unwrap();
    runner.emit(AgentEventPayload::Finished).await.unwrap();

    assert!(matches!(
        rx.recv().await.unwrap().payload,
        AgentEventPayload::Started
    ));
    assert!(
        matches!(rx.recv().await.unwrap().payload, AgentEventPayload::Stream { ref text } if text == "hello")
    );
    assert!(matches!(
        rx.recv().await.unwrap().payload,
        AgentEventPayload::Finished
    ));
}

// --- handle_control behavior tests ---

#[tokio::test]
async fn test_handle_control_clear_resets_state() {
    let (mut runner, mut event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    runner.params.messages.push(Message::user("msg1"));
    runner.params.messages.push(Message::user("msg2"));
    runner.turn_count = 5;
    runner.tokens.input = 1000;
    runner.tokens.output = 500;

    ctrl_tx.send(ControlCommand::Clear).await.unwrap();
    drop(ctrl_tx);

    // wait_for_input processes Clear then blocks on the open mailbox; timeout exits.
    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    assert!(runner.params.messages.is_empty());
    assert_eq!(runner.turn_count, 0);
    assert_eq!(runner.tokens.input, 0);
    assert_eq!(runner.tokens.output, 0);

    let e1 = event_rx.recv().await.unwrap();
    assert!(matches!(e1.payload, AgentEventPayload::AwaitingInput));
    let e2 = event_rx.recv().await.unwrap();
    assert!(matches!(
        e2.payload,
        AgentEventPayload::TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            ..
        }
    ));
}

#[tokio::test]
async fn test_handle_control_compact_keeps_recent() {
    let (mut runner, mut event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    for i in 0..15 {
        runner
            .params
            .messages
            .push(Message::user(&format!("msg{i}")));
    }
    assert_eq!(runner.params.messages.len(), 15);

    ctrl_tx.send(ControlCommand::Compact).await.unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    assert_eq!(runner.params.messages.len(), 10);
    assert_eq!(runner.params.messages[0].text_content(), "msg5");

    // Verify Compacted event was emitted (after AwaitingInput)
    let e1 = event_rx.recv().await.unwrap();
    assert!(matches!(e1.payload, AgentEventPayload::AwaitingInput));
    let e2 = event_rx.recv().await.unwrap();
    assert!(matches!(
        e2.payload,
        AgentEventPayload::Compacted {
            kept: 10,
            removed: 5
        }
    ));
}

#[tokio::test]
async fn test_handle_control_model_switch_updates_model() {
    let (mut runner, _event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    assert_eq!(runner.params.model, "claude-sonnet-4-20250514");

    ctrl_tx
        .send(ControlCommand::ModelSwitch("claude-opus-4-20250514".into()))
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    assert_eq!(runner.params.model, "claude-opus-4-20250514");
}
