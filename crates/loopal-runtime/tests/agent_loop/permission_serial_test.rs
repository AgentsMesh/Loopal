use std::{sync::Arc, time::Duration};

use loopal_protocol::{AgentEvent, AgentEventPayload, InterruptSignal};
use loopal_runtime::agent_loop::{TurnContext, cancel::TurnCancel};
use loopal_tool_api::PermissionMode;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

async fn next_permission_request(rx: &mut mpsc::Receiver<AgentEvent>) -> String {
    loop {
        let event = rx
            .recv()
            .await
            .expect("event channel closed before permission request");
        if let AgentEventPayload::ToolPermissionRequest { id, .. } = event.payload {
            return id;
        }
    }
}

fn two_write_tools() -> Vec<(String, String, serde_json::Value)> {
    vec![
        (
            "write-1".to_string(),
            "Write".to_string(),
            serde_json::json!({"file_path": "/tmp/one", "content": "one"}),
        ),
        (
            "write-2".to_string(),
            "Write".to_string(),
            serde_json::json!({"file_path": "/tmp/two", "content": "two"}),
        ),
    ]
}

#[tokio::test]
async fn manual_permission_requests_in_one_batch_are_serialized() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, permission_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;

    let execution = tokio::spawn(async move {
        let mut turn_ctx = make_turn_ctx();
        in_turn(runner.execute_tools(
            &mut turn_ctx,
            two_write_tools(),
            loopal_runtime::agent_loop::StreamingToolHandle::empty(),
        ))
        .await
    });

    let first = timeout(
        Duration::from_secs(1),
        next_permission_request(&mut event_rx),
    )
    .await
    .expect("first permission request timed out");
    assert_eq!(first, "write-1");

    let premature = timeout(
        Duration::from_millis(50),
        next_permission_request(&mut event_rx),
    )
    .await;
    assert!(
        premature.is_err(),
        "second permission request appeared before the first response"
    );

    permission_tx.send(false).await.unwrap();
    let second = timeout(
        Duration::from_secs(1),
        next_permission_request(&mut event_rx),
    )
    .await
    .expect("second permission request timed out");
    assert_eq!(second, "write-2");
    permission_tx.send(false).await.unwrap();

    let stats = timeout(Duration::from_secs(1), execution)
        .await
        .expect("tool batch did not finish")
        .expect("tool batch task panicked")
        .expect("tool batch failed");
    assert_eq!(stats.denied, 2);
    assert_eq!(stats.approved, 0);
}

#[tokio::test]
async fn cancelling_first_permission_does_not_emit_second_request() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _permission_tx) =
        make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    let interrupt = InterruptSignal::new();
    let (interrupt_tx, _) = watch::channel(0u64);
    let interrupt_tx = Arc::new(interrupt_tx);
    let turn_interrupt = interrupt.clone();
    let turn_interrupt_tx = Arc::clone(&interrupt_tx);

    let execution = tokio::spawn(async move {
        let cancel = TurnCancel::new(turn_interrupt, turn_interrupt_tx);
        let mut turn_ctx = TurnContext::new(0, cancel);
        in_turn(runner.execute_tools(
            &mut turn_ctx,
            two_write_tools(),
            loopal_runtime::agent_loop::StreamingToolHandle::empty(),
        ))
        .await
    });

    let first = timeout(
        Duration::from_secs(1),
        next_permission_request(&mut event_rx),
    )
    .await
    .expect("first permission request timed out");
    assert_eq!(first, "write-1");

    interrupt.signal();
    interrupt_tx.send_modify(|version| *version = version.wrapping_add(1));
    let stats = timeout(Duration::from_secs(1), execution)
        .await
        .expect("cancelled tool batch did not finish")
        .expect("tool batch task panicked")
        .expect("cancelled tool batch failed");

    let mut later_requests = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        if let AgentEventPayload::ToolPermissionRequest { id, .. } = event.payload {
            later_requests.push(id);
        }
    }
    assert!(
        later_requests.is_empty(),
        "permission requests emitted after cancellation: {later_requests:?}"
    );
    assert_eq!(stats.denied, 2);
    assert_eq!(stats.approved, 0);
}
