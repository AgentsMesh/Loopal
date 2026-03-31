use loopal_error::{LoopalError, TerminateReason};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{StopReason, StreamChunk};

use super::mock_provider::{
    make_interactive_multi_runner, make_multi_runner, make_runner_with_mock_provider,
};

#[tokio::test]
async fn test_full_run_stream_error_recovery_with_close() {
    // Tests stream_error && tool_uses.is_empty() && assistant_text.is_empty()
    // Then the wait_for_input channel is closed, so it breaks.
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

    // Send initial message via mailbox (agent starts with empty store)
    mbox_tx
        .send(loopal_protocol::Envelope::new(
            loopal_protocol::MessageSource::Human,
            "main",
            "go",
        ))
        .await
        .unwrap();

    // Drop senders: after response, wait_for_input sees closed channels -> exits
    drop(mbox_tx);
    drop(ctrl_tx);

    // Drain events in background
    let events = tokio::spawn(async move {
        let mut payloads = vec![];
        while let Some(e) = event_rx.recv().await {
            payloads.push(e.payload);
        }
        payloads
    });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    drop(runner); // Close event channel so the collector finishes
    let payloads = events.await.unwrap();

    // Key assertion: AwaitingInput was emitted AFTER the turn
    assert!(
        payloads
            .iter()
            .any(|p| matches!(p, AgentEventPayload::AwaitingInput)),
        "interactive agent should emit AwaitingInput after turn"
    );
}

/// Prompt-driven session (store has initial messages) exits after one turn
/// without waiting for more input — no AwaitingInput emitted.
#[tokio::test]
async fn test_prompt_driven_exits_after_turn() {
    // Single text response — agent should process and exit
    let calls = vec![vec![
        Ok(StreamChunk::Text {
            text: "Done.".into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    // store already has "go" message -> initial_prompt = true

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

    // Prompt-driven: should NOT emit AwaitingInput (exits after turn)
    assert!(
        !payloads
            .iter()
            .any(|p| matches!(p, AgentEventPayload::AwaitingInput)),
        "prompt-driven agent should exit without AwaitingInput"
    );
    // Should have streamed text
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
    // Agent exits (doesn't hang waiting for input) regardless of error type
    assert!(
        matches!(
            output.terminate_reason,
            TerminateReason::Goal | TerminateReason::Error
        ),
        "prompt-driven error should exit, got: {:?}",
        output.terminate_reason
    );
}
