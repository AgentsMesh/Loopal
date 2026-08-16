use loopal_protocol::{AgentEventPayload, ControlCommand};
use loopal_provider_api::{SharedThinkingConfig, ThinkingConfig};

use super::make_runner_with_channels;

#[tokio::test]
async fn thinking_switch_updates_auxiliary_request_state() {
    let state = SharedThinkingConfig::new(ThinkingConfig::Auto);
    let reader = state.reader();
    let (mut runner, mut event_rx, _mailbox_tx, control_tx, _permission_tx) =
        make_runner_with_channels();
    runner.params.config.thinking_state = Some(state);
    let json = serde_json::to_string(&ThinkingConfig::Disabled).unwrap();
    control_tx
        .send(ControlCommand::ThinkingSwitch(json.clone()))
        .await
        .unwrap();
    drop(control_tx);

    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runner.wait_for_input(),
    )
    .await;

    assert!(matches!(reader.get(), ThinkingConfig::Disabled));
    assert!(matches!(
        runner.model_config.thinking,
        ThinkingConfig::Disabled
    ));
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(
        event.payload,
        AgentEventPayload::ThinkingChanged { thinking_config } if thinking_config == json
    ));
}
