use loopagent_types::error::LoopAgentError;

use super::mock_provider::make_runner_with_mock_provider;

#[tokio::test]
async fn test_full_run_stream_error_recovery_with_close() {
    // Tests stream_error && tool_uses.is_empty() && assistant_text.is_empty()
    // Then the wait_for_input channel is closed, so it breaks.
    let chunks = vec![
        Err(LoopAgentError::Provider(loopagent_types::error::ProviderError::StreamEnded)),
    ];
    let (mut runner, mut event_rx, input_tx) = make_runner_with_mock_provider(chunks);

    drop(input_tx);

    tokio::spawn(async move {
        while event_rx.recv().await.is_some() {}
    });

    let result = runner.run().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_full_run_max_turns_with_messages_present() {
    // Tests turn_count >= max_turns with messages already present
    let chunks = vec![];
    let (mut runner, mut event_rx, input_tx) = make_runner_with_mock_provider(chunks);
    runner.params.max_turns = 0;

    drop(input_tx);

    tokio::spawn(async move {
        while event_rx.recv().await.is_some() {}
    });

    let result = runner.run().await;
    assert!(result.is_ok());
}
