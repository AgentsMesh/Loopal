use loopal_error::LoopalError;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{StopReason, StreamChunk};
use loopal_runtime::AgentMode;

use super::{in_turn, make_cancel, make_runner, make_runner_with_mock_provider};

#[test]
fn test_prepare_chat_params_act_mode() {
    let (runner, _rx) = make_runner();
    let params = runner.prepare_chat_params(None).expect("should succeed");

    assert_eq!(params.model, "claude-sonnet-4-20250514");
    // Default system_prompt is empty; only env section is appended
    assert!(
        !params.system_prompt.is_empty(),
        "env section should be present"
    );
    // With empty messages and 200K window, max_tokens should be preserved (headroom is large).
    assert_eq!(params.max_tokens, runner.model_config.max_output_tokens);
    assert!(params.turns.is_empty());
    // Builtin tools should be present
    assert!(!params.tools.is_empty());
}

#[test]
fn test_prepare_chat_params_plan_mode_passes_through() {
    // Mode is now handled by fragment system at prompt build time,
    // not appended by llm.rs. Verify system_prompt starts with the original
    // (env section is appended dynamically per-turn).
    let (mut runner, _rx) = make_runner();
    runner.params.config.mode = AgentMode::Plan;
    let params = runner.prepare_chat_params(None).expect("should succeed");

    assert!(
        params
            .system_prompt
            .starts_with(&runner.params.config.system_prompt),
        "llm.rs should preserve original system_prompt (env section appended)"
    );
}

#[test]
fn test_prepare_chat_params_with_turns() {
    use loopal_turn::{Turn, TurnTrigger};

    let (mut runner, _rx) = make_runner();
    runner.turns.store_mut().start_turn(TurnTrigger::UserInput {
        envelope_id: "env-1".into(),
        content: "Hello".into(),
        images: Vec::new(),
    });
    // pre-existing turn in store (no need to actively use start_turn_record
    // here; we just exercise the prepare_chat_params projection).

    let params = runner.prepare_chat_params(None).expect("should succeed");
    assert_eq!(params.turns.len(), 1);
    assert!(matches!(
        params.turns[0].trigger,
        TurnTrigger::UserInput { ref content, .. } if content == "Hello"
    ));
    let _: &Turn = &params.turns[0];
}

#[tokio::test]
async fn test_stream_llm_text_response() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "Hello ".to_string(),
        }),
        Ok(StreamChunk::Text {
            text: "world!".to_string(),
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
    let (mut runner, mut event_rx, _input_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);

    let cancel = make_cancel();
    let result = in_turn(runner.stream_llm_with(None, &cancel))
        .await
        .unwrap();
    let text = result.assistant_text;
    let tool_uses = result.tool_uses;
    let stream_error = result.stream_error;
    assert_eq!(text, "Hello world!");
    assert!(tool_uses.is_empty());
    assert!(!stream_error);
    assert_eq!(runner.tokens.input, 10);
    assert_eq!(runner.tokens.output, 5);
    assert!(runner.tokens.input > 0);

    // Drain events and verify
    let mut events = Vec::new();
    while let Ok(e) = event_rx.try_recv() {
        events.push(e);
    }
    assert!(
        events.iter().any(
            |e| matches!(e.payload, AgentEventPayload::Stream { ref text } if text == "Hello ")
        )
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e.payload, AgentEventPayload::TokenUsage { .. }))
    );
}

#[tokio::test]
async fn test_stream_llm_tool_use_response() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "Let me read.".to_string(),
        }),
        Ok(StreamChunk::ToolUse {
            id: "tc-1".to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({"file_path": "/tmp/test.rs"}),
        }),
        Ok(StreamChunk::Usage {
            input_tokens: 20,
            output_tokens: 10,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ];
    let (mut runner, _event_rx, _input_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);

    let cancel = make_cancel();
    let result = in_turn(runner.stream_llm_with(None, &cancel))
        .await
        .unwrap();
    let text = result.assistant_text;
    let tool_uses = result.tool_uses;
    let stream_error = result.stream_error;
    assert_eq!(text, "Let me read.");
    assert_eq!(tool_uses.len(), 1);
    assert_eq!(tool_uses[0].0, "tc-1");
    assert_eq!(tool_uses[0].1, "Read");
    assert!(!stream_error);
}

#[tokio::test]
async fn test_stream_llm_error_in_stream() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "partial".to_string(),
        }),
        Err(LoopalError::Provider(
            loopal_error::ProviderError::StreamEnded,
        )),
    ];
    let (mut runner, _event_rx, _input_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);

    let cancel = make_cancel();
    let result = in_turn(runner.stream_llm_with(None, &cancel))
        .await
        .unwrap();
    let text = result.assistant_text;
    let tool_uses = result.tool_uses;
    let stream_error = result.stream_error;
    assert_eq!(text, "partial");
    assert!(tool_uses.is_empty());
    assert!(stream_error);
}

#[tokio::test]
async fn test_stream_llm_empty_stream() {
    // Empty stream (no chunks at all) — stream EOF without Done = truncation.
    let chunks = vec![];
    let (mut runner, _event_rx, _input_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);

    let cancel = make_cancel();
    let result = in_turn(runner.stream_llm_with(None, &cancel))
        .await
        .unwrap();
    let text = result.assistant_text;
    let tool_uses = result.tool_uses;
    let stream_error = result.stream_error;
    assert!(text.is_empty());
    assert!(tool_uses.is_empty());
    assert!(
        stream_error,
        "empty stream (no Done) should set stream_error"
    );
}

#[test]
fn report_real_system_prompt_tokens() {
    let (mut runner, _rx) = make_runner();

    // Build a real system prompt using the fragment system with real tool defs
    let tool_defs = runner.params.deps.kernel.tool_definitions();
    let real_prompt = loopal_context::build_system_prompt(
        "You are a helpful assistant.",
        &tool_defs,
        "act",
        "/Users/dev/project",
        "",
        "",
        None,
        vec![],
        0,
    );
    runner.params.config.system_prompt = real_prompt.clone();
    let params = runner.prepare_chat_params(None).unwrap();

    let tokens = loopal_context::estimate_tokens(&params.system_prompt);

    // Count tool schema portion
    let prompt_no_tools = loopal_context::build_system_prompt(
        "You are a helpful assistant.",
        &[],
        "act",
        "/Users/dev/project",
        "",
        "",
        None,
        vec![],
        0,
    );
    let fragment_tokens = loopal_context::estimate_tokens(&prompt_no_tools);

    eprintln!("\n=== Real System Prompt Token Report ===");
    eprintln!(
        "Total system prompt: {} tokens ({} chars)",
        tokens,
        params.system_prompt.len()
    );
    eprintln!("Tool count:          {}", tool_defs.len());
    eprintln!("Behavior fragments:  {fragment_tokens} tokens");
    eprintln!("Tool schemas:        {} tokens", tokens - fragment_tokens);
}
