use loopal_error::TerminateReason;
use loopal_provider_api::{StopReason, StreamChunk};

use super::mock_provider::make_runner_with_mock_provider;

/// Non-interactive runner: text-only response → turn ends, loop exits with Goal.
#[tokio::test]
async fn test_turn_text_only_non_interactive() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "Done!".to_string(),
        }),
        Ok(StreamChunk::Usage {
            input_tokens: 5,
            output_tokens: 3,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ];
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(output.result, "Done!");
}

/// Non-interactive runner with tool → LLM → no tools: single turn, no redundant LLM call.
#[tokio::test]
async fn test_turn_tool_then_text_non_interactive() {
    let tmp_file = std::env::temp_dir().join(format!("la_turn_test_{}.txt", std::process::id()));
    std::fs::write(&tmp_file, "content").unwrap();

    // MockProvider only yields one batch of chunks, so if the runner tries
    // a second LLM call (the old bug), stream_llm would get an empty stream.
    // The inner loop should: LLM(tool) → execute_tool → hit max_turns → exit.
    let chunks = vec![
        Ok(StreamChunk::ToolUse {
            id: "tc-1".to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({"file_path": tmp_file.to_str().unwrap()}),
        }),
        Ok(StreamChunk::Usage {
            input_tokens: 10,
            output_tokens: 5,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ];
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);
    runner.params.config.max_turns = 1;

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // Tool executed, then max_turns hit inside execute_turn → outer loop breaks
    assert!(runner.params.store.len() >= 3);
    // Result may be empty since LLM text was empty (only tool use in the stream)
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    let _ = std::fs::remove_file(&tmp_file);
}

/// Non-interactive: stream error with no prior output → Goal with empty result.
/// Matches old behavior: stream_error + no content → break → Ok("").
#[tokio::test]
async fn test_turn_stream_error_no_prior_output() {
    let chunks = vec![Err(loopal_error::LoopalError::Provider(
        loopal_error::ProviderError::StreamEnded,
    ))];
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // Stream error with no prior text → Goal (not Error), matching old break behavior
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert!(output.result.is_empty());
}
