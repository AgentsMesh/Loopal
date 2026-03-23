use loopal_error::LoopalError;
use loopal_message::MessageRole;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{StopReason, StreamChunk};
use loopal_runtime::AgentMode;

use super::{make_cancel, make_runner, make_runner_with_mock_provider};

#[test]
fn test_prepare_chat_params_act_mode() {
    let (runner, _rx) = make_runner();
    let params = runner
        .prepare_chat_params_with(&runner.params.messages)
        .expect("should succeed");

    assert_eq!(params.model, "claude-sonnet-4-20250514");
    assert!(
        params
            .system_prompt
            .contains("You are a helpful assistant.")
    );
    assert_eq!(params.max_tokens, runner.model_config.max_output_tokens);
    assert!(params.messages.is_empty());
    // Builtin tools should be present
    assert!(!params.tools.is_empty());
}

#[test]
fn test_prepare_chat_params_plan_mode_has_suffix() {
    let (mut runner, _rx) = make_runner();
    runner.params.mode = AgentMode::Plan;
    let params = runner
        .prepare_chat_params_with(&runner.params.messages)
        .expect("should succeed");

    assert!(
        params.system_prompt.contains("PLAN mode"),
        "plan mode should append suffix to system prompt"
    );
    assert!(
        params
            .system_prompt
            .starts_with("You are a helpful assistant.")
    );
}

#[test]
fn test_prepare_chat_params_with_messages() {
    use loopal_message::Message;

    let (mut runner, _rx) = make_runner();
    runner.params.messages.push(Message::user("Hello"));
    runner.params.messages.push(Message::assistant("Hi there!"));

    let params = runner
        .prepare_chat_params_with(&runner.params.messages)
        .expect("should succeed");
    assert_eq!(params.messages.len(), 2);
    assert_eq!(params.messages[0].role, MessageRole::User);
    assert_eq!(params.messages[1].role, MessageRole::Assistant);
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

    let msgs = runner.params.messages.clone();
    let cancel = make_cancel();
    let result = runner.stream_llm_with(&msgs, &cancel).await.unwrap();
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

    let msgs = runner.params.messages.clone();
    let cancel = make_cancel();
    let result = runner.stream_llm_with(&msgs, &cancel).await.unwrap();
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

    let msgs = runner.params.messages.clone();
    let cancel = make_cancel();
    let result = runner.stream_llm_with(&msgs, &cancel).await.unwrap();
    let text = result.assistant_text;
    let tool_uses = result.tool_uses;
    let stream_error = result.stream_error;
    assert_eq!(text, "partial");
    assert!(tool_uses.is_empty());
    assert!(stream_error);
}

#[tokio::test]
async fn test_stream_llm_empty_stream() {
    // Empty stream (no chunks at all) — tests the while loop body never executing
    let chunks = vec![];
    let (mut runner, _event_rx, _input_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);

    let msgs = runner.params.messages.clone();
    let cancel = make_cancel();
    let result = runner.stream_llm_with(&msgs, &cancel).await.unwrap();
    let text = result.assistant_text;
    let tool_uses = result.tool_uses;
    let stream_error = result.stream_error;
    assert!(text.is_empty());
    assert!(tool_uses.is_empty());
    assert!(!stream_error);
}
