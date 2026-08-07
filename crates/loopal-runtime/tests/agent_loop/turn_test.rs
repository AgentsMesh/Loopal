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
    let (mut runner, mut event_rx, mbox_tx, ctrl_tx) = make_runner_with_mock_provider(chunks);
    // Drop senders so recv_input() returns None after the turn completes.
    drop(mbox_tx);
    drop(ctrl_tx);

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

    // Tool results require one follow-up model call in the same turn. Keep the
    // fixture protocol-complete so an exhausted mock queue cannot masquerade as
    // a valid empty response (or burn the production EOF retry budget).
    let calls = vec![
        vec![
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
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "Read complete.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = super::mock_provider::make_multi_runner(calls);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // Tool executed, non-interactive agent exits after first turn
    assert!(runner.turns.view().len() >= 3);
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(output.result, "Read complete.");

    let _ = std::fs::remove_file(&tmp_file);
}

/// Non-interactive: a stream error with no prior output exhausts its retry
/// budget and terminates as Error, never an empty Goal.
#[tokio::test(start_paused = true)]
async fn test_turn_stream_error_no_prior_output() {
    let calls = (0..7)
        .map(|_| {
            vec![Err(loopal_error::LoopalError::Provider(
                loopal_error::ProviderError::StreamEnded,
            ))]
        })
        .collect();
    let (mut runner, mut event_rx) = super::mock_provider::make_multi_runner(calls);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Error);
    assert!(output.result.contains("Stream ended unexpectedly"));
}

/// E2E: AskUser + Read in the same LLM response → each tool_use_id appears
/// exactly once in the stored tool_result message. Regression test for the
/// duplicate tool_result bug where RunnerDirect tools were early-started AND
/// intercepted, producing two ToolResult blocks with the same id.
#[tokio::test]
async fn ask_user_plus_read_no_duplicate_via_run() {
    use std::collections::HashSet;

    let tmp = std::env::temp_dir().join(format!("la_dispatch_e2e_{}.txt", std::process::id()));
    std::fs::write(&tmp, "e2e content").unwrap();

    let calls = vec![
        // Call 1: LLM returns AskUser + Read tool calls
        vec![
            Ok(StreamChunk::ToolUse {
                id: "tc-ask".to_string(),
                name: "AskUser".to_string(),
                input: serde_json::json!({
                    "questions": [{
                        "question": "Pick",
                        "options": [
                            {"label": "A", "description": "a"},
                            {"label": "B", "description": "b"}
                        ]
                    }]
                }),
            }),
            Ok(StreamChunk::ToolUse {
                id: "tc-read".to_string(),
                name: "Read".to_string(),
                input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
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
        ],
        // Call 2: LLM produces final text
        vec![
            Ok(StreamChunk::Text {
                text: "Done.".to_string(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];

    let (mut runner, mut event_rx) = super::mock_provider::make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    runner.run().await.unwrap();

    // Find the User message that contains tool results.
    let tool_result_msg = runner
        .turns
        .view()
        .messages()
        .iter()
        .find(|m| {
            m.role == loopal_provider_api::MessageRole::User
                && m.content
                    .iter()
                    .any(|b| matches!(b, loopal_provider_api::ContentBlock::ToolResult { .. }))
        })
        .expect("expected a User message with ToolResult blocks");

    let tool_ids: Vec<&str> = tool_result_msg
        .content
        .iter()
        .filter_map(|b| match b {
            loopal_provider_api::ContentBlock::ToolResult { tool_use_id, .. } => {
                Some(tool_use_id.as_str())
            }
            _ => None,
        })
        .collect();

    // Must have exactly 2 unique tool_use_ids — no duplicates.
    let unique: HashSet<&str> = tool_ids.iter().copied().collect();
    assert_eq!(
        tool_ids.len(),
        unique.len(),
        "duplicate tool_use_id found: {tool_ids:?}"
    );
    assert_eq!(tool_ids.len(), 2);

    // Verify AskUser result doesn't contain the fallback execute() text.
    for block in &tool_result_msg.content {
        if let loopal_provider_api::ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } = block
            && tool_use_id == "tc-ask"
        {
            assert!(
                !content.contains("intercepted by runner"),
                "AskUser fallback leaked into tool_result: {content}"
            );
        }
    }

    let _ = std::fs::remove_file(&tmp);
}
