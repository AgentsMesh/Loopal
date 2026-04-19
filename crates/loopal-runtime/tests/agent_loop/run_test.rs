use loopal_error::{LoopalError, TerminateReason};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{StopReason, StreamChunk};

use super::mock_provider::{
    make_interactive_multi_runner, make_multi_runner, make_runner_with_mock_provider,
};

#[tokio::test]
async fn test_full_run_stream_error_recovery_with_close() {
    let chunks = vec![Err(LoopalError::Provider(
        loopal_error::ProviderError::StreamEnded,
    ))];
    let (mut runner, mut event_rx, input_tx, ctrl_tx) = make_runner_with_mock_provider(chunks);

    drop(input_tx);
    drop(ctrl_tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let result = runner.run().await;
    assert!(result.is_ok());
}

/// Interactive agent emits AwaitingInput after responding, then exits when
/// channels close.
#[tokio::test]
async fn test_interactive_emits_awaiting_input() {
    let calls = vec![vec![
        Ok(StreamChunk::Text {
            text: "all done".into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx, mbox_tx, ctrl_tx) =
        make_interactive_multi_runner(calls, |_k| {});

    mbox_tx
        .send(loopal_protocol::Envelope::new(
            loopal_protocol::MessageSource::Human,
            "main",
            "go",
        ))
        .await
        .unwrap();

    drop(mbox_tx);
    drop(ctrl_tx);

    let events = tokio::spawn(async move {
        let mut payloads = vec![];
        while let Some(e) = event_rx.recv().await {
            payloads.push(e.payload);
        }
        payloads
    });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    drop(runner);
    let payloads = events.await.unwrap();

    assert!(
        payloads
            .iter()
            .any(|p| matches!(p, AgentEventPayload::AwaitingInput)),
        "agent should emit AwaitingInput after turn"
    );
}

/// Prompt-driven session: store has pre-loaded messages, agent processes them,
/// enters idle (emits AwaitingInput), then exits when channel is closed.
#[tokio::test]
async fn test_prompt_driven_exits_after_turn() {
    let calls = vec![vec![
        Ok(StreamChunk::Text {
            text: "Done.".into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    // make_multi_runner pre-loads "go" in store and drops input senders.
    let (mut runner, mut event_rx) = make_multi_runner(calls);

    let events = tokio::spawn(async move {
        let mut payloads = vec![];
        while let Some(e) = event_rx.recv().await {
            payloads.push(e.payload);
        }
        payloads
    });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    drop(runner);
    let payloads = events.await.unwrap();

    // Unified behavior: ALL agents emit AwaitingInput after turn completion.
    assert!(
        payloads
            .iter()
            .any(|p| matches!(p, AgentEventPayload::AwaitingInput)),
        "prompt-driven agent should emit AwaitingInput (unified idle signal)"
    );
    assert!(
        payloads
            .iter()
            .any(|p| matches!(p, AgentEventPayload::Stream { .. })),
        "should have Stream event"
    );
}

/// Prompt-driven session with LLM error -> exits cleanly (no hang).
#[tokio::test]
async fn test_prompt_driven_error_exits_cleanly() {
    let calls = vec![vec![Err(LoopalError::Provider(
        loopal_error::ProviderError::Http("connection refused".into()),
    ))]];
    let (mut runner, _event_rx) = make_multi_runner(calls);

    let output = runner.run().await.unwrap();
    assert!(
        matches!(
            output.terminate_reason,
            TerminateReason::Goal | TerminateReason::Error
        ),
        "prompt-driven error should exit, got: {:?}",
        output.terminate_reason
    );
}

/// Authoritative `Running` event is emitted before any `Stream`, so the
/// TUI status bar can flip before the first LLM byte arrives.
#[tokio::test]
async fn test_running_emitted_before_stream() {
    let calls = vec![vec![
        Ok(StreamChunk::Text {
            text: "hello".into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx) = make_multi_runner(calls);

    let events = tokio::spawn(async move {
        let mut payloads = vec![];
        while let Some(e) = event_rx.recv().await {
            payloads.push(e.payload);
        }
        payloads
    });

    runner.run().await.unwrap();
    drop(runner);
    let payloads = events.await.unwrap();

    let running_pos = payloads
        .iter()
        .position(|p| matches!(p, AgentEventPayload::Running));
    let stream_pos = payloads
        .iter()
        .position(|p| matches!(p, AgentEventPayload::Stream { .. }));
    assert!(running_pos.is_some(), "Running event must be emitted");
    assert!(stream_pos.is_some(), "Stream event must be emitted");
    assert!(
        running_pos.unwrap() < stream_pos.unwrap(),
        "Running must precede first Stream",
    );
}
