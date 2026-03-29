//! Tests for scheduler-related input handling (scheduled messages, trigger_rx).

use loopal_protocol::{Envelope, MessageSource};

use super::make_runner_with_channels;

#[tokio::test]
async fn test_wait_for_input_scheduled_message_has_prefix() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    mbox_tx
        .send(Envelope::new(
            MessageSource::Scheduled,
            "main",
            "check deploys",
        ))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());
    assert_eq!(
        runner.params.store.messages()[0].text_content(),
        "[scheduled] check deploys"
    );
}

#[tokio::test]
async fn test_trigger_rx_delivers_scheduled_message() {
    let (mut runner, _event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    let (trigger_tx, trigger_rx) = tokio::sync::mpsc::channel(16);
    runner.trigger_rx = Some(trigger_rx);

    trigger_tx
        .send(Envelope::new(
            MessageSource::Scheduled,
            "self",
            "run health check",
        ))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());
    assert_eq!(
        runner.params.store.messages()[0].text_content(),
        "[scheduled] run health check"
    );
}

#[tokio::test]
async fn test_trigger_rx_closed_does_not_exit_agent() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    let (trigger_tx, trigger_rx) = tokio::sync::mpsc::channel::<loopal_protocol::Envelope>(16);
    runner.trigger_rx = Some(trigger_rx);
    drop(trigger_tx); // close trigger channel

    // Send a real frontend message so the loop has something to return.
    mbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "hello"))
        .await
        .unwrap();

    // Should NOT return None (agent exit). The trigger channel closing
    // is non-fatal — agent continues to process frontend messages.
    let result = runner.wait_for_input().await.unwrap();
    assert!(
        result.is_some(),
        "agent should not exit when trigger_rx closes"
    );
}
