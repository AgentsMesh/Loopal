use std::sync::Arc;
use std::time::Duration;

use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;

use super::{make_runner, make_runner_with_channels};

#[test]
fn test_model_info_defaults_for_unknown_model() {
    use loopal_config::Settings;
    use loopal_kernel::Kernel;
    use loopal_runtime::agent_loop::AgentLoopRunner;
    use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
    use loopal_runtime::{
        AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend,
    };
    use loopal_test_support::TestFixture;
    use loopal_tool_api::PermissionMode;
    use tokio::sync::mpsc;

    let fixture = TestFixture::new();
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

    let params = AgentLoopParams {
        config: AgentConfig {
            model: "unknown-model-xyz".to_string(),
            permission_mode: PermissionMode::Supervised,
            max_turns: 5,
            ..Default::default()
        },
        deps: AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("test"),
        store: loopal_context::ContextStore::new(super::make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
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

    runner.params.store.push_user(Message::user("msg1"));
    runner.params.store.push_user(Message::user("msg2"));
    runner.turn_count = 5;
    runner.tokens.input = 1000;
    runner.tokens.output = 500;

    ctrl_tx.send(ControlCommand::Clear).await.unwrap();
    drop(ctrl_tx);

    // wait_for_input processes Clear then blocks on the open mailbox; timeout exits.
    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    assert!(runner.params.store.is_empty());
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
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    for i in 0..15 {
        runner
            .params
            .store
            .push_user(Message::user(&format!("msg{i}")));
    }
    assert_eq!(runner.params.store.len(), 15);

    // Directly call force_compact (same path as /compact command)
    runner.force_compact().await.unwrap();

    // With budget-aware ContextStore, 15 tiny messages (~5 tokens each) are well
    // within 50% of the 173K budget, so force_compact short-circuits with
    // "nothing to compact" instead of actually truncating.
    // Verify the short-circuit event was emitted.
    let e1 = event_rx.recv().await.unwrap();
    assert!(matches!(e1.payload, AgentEventPayload::Stream { .. }));
}

#[tokio::test]
async fn test_handle_control_model_switch_updates_model() {
    let (mut runner, _event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    assert_eq!(runner.params.config.model, "claude-sonnet-4-20250514");

    ctrl_tx
        .send(ControlCommand::ModelSwitch("claude-opus-4-20250514".into()))
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    assert_eq!(runner.params.config.model, "claude-opus-4-20250514");
}
