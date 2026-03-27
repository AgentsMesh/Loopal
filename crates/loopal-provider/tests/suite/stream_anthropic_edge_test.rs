use super::stream_helpers::{collect_chunks, test_chat_params};
use loopal_provider::AnthropicProvider;
use loopal_provider_api::{Provider, StreamChunk};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_sse_empty_data_handled() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n\
data: \n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"OK\"}}\n\n\
data: {\"type\":\"message_stop\"}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = AnthropicProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let mut got_text = false;
    let mut got_done = false;
    for chunk in &chunks {
        match chunk {
            Ok(StreamChunk::Text { text }) if text == "OK" => got_text = true,
            Ok(StreamChunk::Done { .. }) => got_done = true,
            _ => {} // parse errors from empty data are acceptable
        }
    }
    assert!(got_text, "expected Text(\"OK\") chunk");
    assert!(got_done, "expected Done chunk");
}

#[tokio::test]
async fn test_stream_code_execution_input_via_deltas() {
    let mock_server = MockServer::start().await;

    // Simulate code_execution: empty input at block_start, code streamed via deltas.
    let sse_body = "\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n\
data: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"server_tool_use\",\"id\":\"stu_1\",\"name\":\"code_execution\",\"input\":{}}}\n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"code\\\":\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"print(42)\\\",\\\"language\\\":\\\"python\\\"}\"}}\n\n\
data: {\"type\":\"content_block_stop\"}\n\n\
data: {\"type\":\"message_stop\"}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = AnthropicProvider::new("test-key".to_string()).with_base_url(mock_server.uri());
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;

    let mut found = false;
    for chunk in chunks.into_iter().flatten() {
        if let StreamChunk::ServerToolUse { id, name, input } = chunk {
            assert_eq!(id, "stu_1");
            assert_eq!(name, "code_execution");
            assert_eq!(input["code"], "print(42)");
            assert_eq!(input["language"], "python");
            found = true;
        }
    }
    assert!(
        found,
        "expected ServerToolUse with reconstructed input from deltas"
    );
}

#[tokio::test]
async fn test_stream_server_tool_preserves_start_input_without_deltas() {
    let mock_server = MockServer::start().await;

    // web_search: full input at block_start, no input_json_delta events.
    let sse_body = "\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\n\
data: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"server_tool_use\",\"id\":\"stu_ws\",\"name\":\"web_search\",\"input\":{\"query\":\"rust async\"}}}\n\n\
data: {\"type\":\"content_block_stop\"}\n\n\
data: {\"type\":\"message_stop\"}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = AnthropicProvider::new("test-key".to_string()).with_base_url(mock_server.uri());
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;

    let mut found = false;
    for chunk in chunks.into_iter().flatten() {
        if let StreamChunk::ServerToolUse { id, name, input } = chunk {
            assert_eq!(id, "stu_ws");
            assert_eq!(name, "web_search");
            assert_eq!(input["query"], "rust async");
            found = true;
        }
    }
    assert!(found, "expected ServerToolUse with input from block_start");
}

#[tokio::test]
async fn test_stream_server_tool_deltas_replace_block_start_input() {
    let mock_server = MockServer::start().await;

    // Block_start has stale input; deltas carry the real input. Deltas should win.
    let sse_body = "\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\n\
data: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"server_tool_use\",\"id\":\"stu_r\",\"name\":\"code_execution\",\"input\":{\"stale\":true}}}\n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"code\\\":\\\"x=1\\\"}\"}}\n\n\
data: {\"type\":\"content_block_stop\"}\n\n\
data: {\"type\":\"message_stop\"}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = AnthropicProvider::new("test-key".to_string()).with_base_url(mock_server.uri());
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;

    let mut found = false;
    for chunk in chunks.into_iter().flatten() {
        if let StreamChunk::ServerToolUse { input, .. } = chunk {
            // Deltas replace block_start input entirely
            assert_eq!(input["code"], "x=1");
            assert!(
                input.get("stale").is_none(),
                "block_start input should be replaced"
            );
            found = true;
        }
    }
    assert!(found);
}
