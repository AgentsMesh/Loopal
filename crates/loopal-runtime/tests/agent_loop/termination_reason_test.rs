use std::time::Duration;

use loopal_error::{LoopalError, ProviderError, TerminateReason};
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, MessageSource};
use loopal_provider_api::{Message, StopReason, StreamChunk};
use loopal_runtime::LifecycleMode;
use loopal_test_support::{HarnessBuilder, chunks};

async fn wait_for_event(
    rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>,
    label: &str,
    predicate: impl Fn(&AgentEventPayload) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = rx.recv().await {
            if predicate(&event.payload) {
                return;
            }
        }
        panic!("event channel closed while waiting for {label}");
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {label}"));
}

fn fatal_error(message: &str) -> Result<StreamChunk, LoopalError> {
    Err(LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: message.into(),
        retry_after_ms: None,
    }))
}

#[tokio::test]
async fn interrupted_ephemeral_turn_returns_aborted_with_partial_output() {
    let mut harness = HarnessBuilder::new()
        .calls(vec![vec![
            chunks::text("partial result"),
            chunks::text(" must not arrive"),
            chunks::done(),
        ]])
        .messages(vec![Message::user("work")])
        .lifecycle(LifecycleMode::Ephemeral)
        .llm_chunk_delay(Duration::from_millis(500))
        .build()
        .await;

    let mut runner = harness.runner;
    let output_task = tokio::spawn(async move { runner.run().await.unwrap() });

    wait_for_event(&mut harness.event_rx, "first stream chunk", |payload| {
        matches!(payload, AgentEventPayload::Stream { .. })
    })
    .await;
    harness.session_ctrl.interrupt();

    let output = tokio::time::timeout(Duration::from_secs(5), output_task)
        .await
        .expect("interrupted ephemeral runner did not stop")
        .expect("runner task panicked");
    assert_eq!(output.terminate_reason, TerminateReason::Aborted);
    assert_eq!(output.result, "partial result");
}

#[tokio::test]
async fn new_input_after_interrupt_supersedes_aborted_state() {
    let mut harness = HarnessBuilder::new()
        .calls(vec![
            vec![
                chunks::text("interrupted partial"),
                chunks::text(" must not arrive"),
                chunks::done(),
            ],
            chunks::text_turn("recovered result"),
        ])
        .messages(vec![Message::user("first")])
        .lifecycle(LifecycleMode::Ephemeral)
        .llm_chunk_delay(Duration::from_millis(500))
        .build()
        .await;

    let mut runner = harness.runner;
    let output_task = tokio::spawn(async move { runner.run().await.unwrap() });

    wait_for_event(&mut harness.event_rx, "first stream chunk", |payload| {
        matches!(payload, AgentEventPayload::Stream { .. })
    })
    .await;
    harness
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "retry"))
        .await
        .unwrap();
    harness.session_ctrl.interrupt();

    let output = tokio::time::timeout(Duration::from_secs(5), output_task)
        .await
        .expect("ephemeral runner did not process queued replacement input")
        .expect("runner task panicked");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(output.result, "recovered result");
}

#[tokio::test]
async fn fatal_error_after_partial_tool_round_is_error_with_partial_output() {
    let temp = tempfile::tempdir().unwrap();
    let input_path = temp.path().join("input.txt");
    std::fs::write(&input_path, "fixture").unwrap();

    let calls = vec![
        vec![
            chunks::text("partial before tool"),
            chunks::tool_use(
                "read-1",
                "Read",
                serde_json::json!({"file_path": input_path}),
            ),
            chunks::done(),
        ],
        vec![fatal_error("fatal follow-up")],
    ];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("read then answer")])
        .lifecycle(LifecycleMode::Ephemeral)
        .build()
        .await;
    let mut runner = harness.runner;

    let output = runner.run().await.unwrap();

    assert_eq!(output.terminate_reason, TerminateReason::Error);
    assert_eq!(output.result, "partial before tool");
}

#[tokio::test]
async fn successful_persistent_turn_clears_prior_error_state() {
    let mut harness = HarnessBuilder::new()
        .calls(vec![
            vec![fatal_error("first turn failed")],
            vec![Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            })],
        ])
        .messages(vec![])
        .lifecycle(LifecycleMode::Persistent)
        .build()
        .await;

    let mut runner = harness.runner;
    let output_task = tokio::spawn(async move { runner.run().await.unwrap() });
    harness
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "fail"))
        .await
        .unwrap();
    wait_for_event(&mut harness.event_rx, "first-turn error", |payload| {
        matches!(payload, AgentEventPayload::Error { .. })
    })
    .await;

    harness
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "recover"))
        .await
        .unwrap();
    wait_for_event(
        &mut harness.event_rx,
        "successful replacement turn",
        |payload| matches!(payload, AgentEventPayload::TurnCompleted(_)),
    )
    .await;
    drop(harness.mailbox_tx);
    drop(harness.control_tx);
    drop(harness.session_ctrl);

    let output = tokio::time::timeout(Duration::from_secs(5), output_task)
        .await
        .expect("persistent runner did not exit after input channels closed")
        .expect("runner task panicked");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert!(output.result.is_empty());
}
