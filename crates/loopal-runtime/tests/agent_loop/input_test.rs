use loopal_protocol::AgentEventPayload;
use loopal_protocol::ControlCommand;
use loopal_protocol::{Envelope, MessageSource};
use loopal_runtime::AgentMode;

use super::{make_runner, make_runner_with_channels};

#[test]
fn test_agent_loop_runner_construction() {
    let (runner, _rx) = make_runner();
    assert_eq!(runner.turn_count, 0);
    assert_eq!(runner.tokens.input, 0);
    assert_eq!(runner.tokens.output, 0);
    assert_eq!(runner.params.model, "claude-sonnet-4-20250514");
    assert_eq!(runner.params.max_turns, 10);
}

#[test]
fn test_tool_ctx_matches_session() {
    let (runner, _rx) = make_runner();
    assert_eq!(
        runner.tool_ctx.backend.cwd(),
        std::path::Path::new("/tmp")
            .canonicalize()
            .unwrap_or_else(|_| "/tmp".into())
    );
    assert_eq!(runner.tool_ctx.session_id, "test-session-001");
}

#[tokio::test]
async fn test_wait_for_input_human_message_no_prefix() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    mbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "Hello agent"))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());
    assert_eq!(runner.params.store.len(), 1);
    // Human source: no prefix — content passed through directly
    assert_eq!(
        runner.params.store.messages()[0].text_content(),
        "Hello agent"
    );

    // Should have emitted AwaitingInput
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(event.payload, AgentEventPayload::AwaitingInput));
}

#[tokio::test]
async fn test_wait_for_input_agent_message_has_prefix() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    mbox_tx
        .send(Envelope::new(
            MessageSource::Agent("researcher".into()),
            "main",
            "found the answer",
        ))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());
    assert_eq!(
        runner.params.store.messages()[0].text_content(),
        "[from: researcher] found the answer"
    );
}

#[tokio::test]
async fn test_wait_for_input_channel_message_has_channel_prefix() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    mbox_tx
        .send(Envelope::new(
            MessageSource::Channel {
                channel: "general".into(),
                from: "bot".into(),
            },
            "main",
            "new event",
        ))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());
    assert_eq!(
        runner.params.store.messages()[0].text_content(),
        "[from: #general/bot] new event"
    );
}

#[tokio::test]
async fn test_wait_for_input_mode_switch() {
    let (mut runner, mut event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    ctrl_tx
        .send(ControlCommand::ModeSwitch(loopal_protocol::AgentMode::Plan))
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runner.wait_for_input(),
    )
    .await;

    assert_eq!(runner.params.mode, AgentMode::Plan);

    let e1 = event_rx.recv().await.unwrap();
    assert!(matches!(e1.payload, AgentEventPayload::AwaitingInput));
    let e2 = event_rx.recv().await.unwrap();
    assert!(matches!(e2.payload, AgentEventPayload::ModeChanged { ref mode } if mode == "plan"));
}

#[tokio::test]
async fn test_wait_for_input_channel_closed() {
    let (mut runner, _event_rx, mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();
    drop(mbox_tx);
    drop(ctrl_tx);

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_emit_with_open_channel() {
    let (runner, mut rx) = make_runner();

    runner
        .emit(AgentEventPayload::Started)
        .await
        .expect("emit to open channel should succeed");

    let event = rx.recv().await.expect("should receive event");
    assert!(matches!(event.payload, AgentEventPayload::Started));
}

#[tokio::test]
async fn test_emit_with_closed_channel() {
    let (runner, rx) = make_runner();
    drop(rx); // close the receiver

    let result = runner.emit(AgentEventPayload::Started).await;
    assert!(result.is_err(), "emit to closed channel should fail");
}
