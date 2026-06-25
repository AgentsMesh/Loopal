//! Emit-first consistency tests. Every control handler converted to
//! emit-first must leave the runner's internal state untouched when the
//! event emit fails, otherwise agent state and view-state would drift
//! once the receiver reconnects via view/snapshot. Table-driven across
//! the four handlers (Clear / ModelSwitch / ThinkingSwitch / ModeSwitch)
//! so a future regression that reverts any single handler back to
//! mutate-then-emit is caught here.

use std::time::Duration;

use loopal_protocol::ControlCommand;

use super::make_runner_with_channels;

#[tokio::test]
async fn test_clear_bails_out_when_event_emit_fails() {
    // Drop the event receiver before sending Clear. The send within
    // handle_control returns Err; emit-first semantics mean the runner
    // must NOT mutate local state, otherwise agent and view-state would
    // disagree once the receiver reconnects via view/snapshot.
    let (mut runner, event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();
    drop(event_rx);

    runner.turn_count = 3;
    runner.tokens.input = 800;

    ctrl_tx.send(ControlCommand::Clear).await.unwrap();
    drop(ctrl_tx);

    let result = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;
    assert!(
        matches!(result, Ok(Err(_))),
        "expected wait_for_input to surface the emit failure, got {result:?}"
    );

    // State must be untouched — the emit failed before any mutation ran.
    assert_eq!(runner.turn_count, 3);
    assert_eq!(runner.tokens.input, 800);
}

#[tokio::test]
async fn test_model_switch_bails_out_when_event_emit_fails() {
    let (mut runner, event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();
    drop(event_rx);

    let original = runner.params.config.model();
    ctrl_tx
        .send(ControlCommand::ModelSwitch("claude-opus-4-7".into()))
        .await
        .unwrap();
    drop(ctrl_tx);

    let result = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;
    assert!(matches!(result, Ok(Err(_))));
    assert_eq!(runner.params.config.model(), original);
}

#[tokio::test]
async fn test_thinking_switch_bails_out_when_event_emit_fails() {
    let (mut runner, event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();
    drop(event_rx);

    let original_thinking = runner.model_config.thinking.clone();
    let json = serde_json::json!({"type": "disabled"}).to_string();
    ctrl_tx
        .send(ControlCommand::ThinkingSwitch(json))
        .await
        .unwrap();
    drop(ctrl_tx);

    let result = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;
    assert!(matches!(result, Ok(Err(_))));
    assert_eq!(
        serde_json::to_string(&runner.model_config.thinking).unwrap(),
        serde_json::to_string(&original_thinking).unwrap(),
    );
}

#[tokio::test]
async fn test_mode_switch_bails_out_when_event_emit_fails() {
    use loopal_runtime::AgentMode as RuntimeMode;

    let (mut runner, event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();
    drop(event_rx);

    runner.params.config.mode = RuntimeMode::Act;
    ctrl_tx
        .send(ControlCommand::ModeSwitch(loopal_protocol::AgentMode::Plan))
        .await
        .unwrap();
    drop(ctrl_tx);

    let result = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;
    assert!(matches!(result, Ok(Err(_))));
    assert_eq!(runner.params.config.mode, RuntimeMode::Act);
}
