use super::stream_helpers::{collect_chunks, test_chat_params};
use loopal_error::{LoopalError, ProviderError};
use loopal_provider::AnthropicProvider;
use loopal_provider_api::{Provider, StreamChunk};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn expect_err(result: Result<loopal_provider_api::ChatStream, LoopalError>) -> LoopalError {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err, got Ok"),
    }
}

#[tokio::test]
async fn test_anthropic_stream_chat_success() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n\
data: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n\
data: {\"type\":\"content_block_stop\"}\n\n\
data: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":0,\"output_tokens\":5}}\n\n\
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
            Ok(StreamChunk::Text { text }) => {
                assert_eq!(text, "Hello");
                got_text = true;
            }
            Ok(StreamChunk::Done { .. }) => got_done = true,
            Ok(StreamChunk::Usage { .. }) => {}
            other => panic!("unexpected chunk: {other:?}"),
        }
    }
    assert!(got_text, "expected a Text chunk");
    assert!(got_done, "expected a Done chunk");
}

#[tokio::test]
async fn test_anthropic_stream_chat_with_tool_use() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n\
data: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool_1\",\"name\":\"read_file\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"main.rs\\\"}\"}}\n\n\
data: {\"type\":\"content_block_stop\"}\n\n\
data: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":0,\"output_tokens\":15}}\n\n\
data: {\"type\":\"message_stop\"}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = AnthropicProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let mut got_tool_use = false;
    for chunk in chunks.into_iter().flatten() {
        if let StreamChunk::ToolUse { id, name, input } = chunk {
            assert_eq!(id, "tool_1");
            assert_eq!(name, "read_file");
            assert_eq!(input["path"], "main.rs");
            got_tool_use = true;
        }
    }
    assert!(got_tool_use, "expected a ToolUse chunk");
}

#[tokio::test]
async fn test_anthropic_stream_chat_rate_limited() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "5")
                .set_body_string("rate limited"),
        )
        .mount(&mock_server)
        .await;

    let provider = AnthropicProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let result = provider.stream_chat(&test_chat_params()).await;
    let err = expect_err(result);
    match &err {
        LoopalError::Provider(ProviderError::RateLimited { retry_after_ms }) => {
            assert_eq!(*retry_after_ms, 5000);
        }
        other => panic!("expected RateLimited error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_anthropic_stream_chat_server_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(500).set_body_string("{\"error\":\"internal server error\"}"),
        )
        .mount(&mock_server)
        .await;

    let provider = AnthropicProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let result = provider.stream_chat(&test_chat_params()).await;
    match expect_err(result) {
        LoopalError::Provider(ProviderError::Api { status, message }) => {
            assert_eq!(status, 500);
            assert!(message.contains("internal server error"));
        }
        other => panic!("expected Api error, got: {other:?}"),
    }
}
