use super::stream_helpers::{collect_chunks, test_chat_params};
use loopal_error::{LoopalError, ProviderError};
use loopal_provider::OpenAiProvider;
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
async fn test_openai_stream_chat_success() {
    let mock_server = MockServer::start().await;

    // Faithful to real OpenAI Responses API wire format: event: prefix + data: JSON
    let sse_body = "\
event: response.output_text.delta\n\
data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\n\n\
event: response.completed\n\
data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"total_tokens\":15}}}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let mut got_text = false;
    let mut got_done = false;
    let mut got_usage = false;
    for chunk in &chunks {
        match chunk {
            Ok(StreamChunk::Text { text }) => {
                assert_eq!(text, "Hi");
                got_text = true;
            }
            Ok(StreamChunk::Done { .. }) => got_done = true,
            Ok(StreamChunk::Usage {
                input_tokens,
                output_tokens,
                ..
            }) => {
                assert_eq!(*input_tokens, 10);
                assert_eq!(*output_tokens, 5);
                got_usage = true;
            }
            other => panic!("unexpected chunk: {other:?}"),
        }
    }
    assert!(got_text, "expected a Text chunk");
    assert!(got_done, "expected a Done chunk");
    assert!(got_usage, "expected a Usage chunk");
}

#[tokio::test]
async fn test_openai_stream_chat_tool_calls() {
    let mock_server = MockServer::start().await;

    // Responses API: function_call comes as output_item.done
    let sse_body = "\
event: response.output_item.done\n\
data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_abc\",\"name\":\"bash\",\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}}\n\n\
event: response.completed\n\
data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"total_tokens\":15}}}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let mut got_tool_use = false;
    for chunk in chunks.into_iter().flatten() {
        if let StreamChunk::ToolUse { id, name, input } = chunk {
            assert_eq!(id, "call_abc");
            assert_eq!(name, "bash");
            assert_eq!(input["cmd"], "ls");
            got_tool_use = true;
        }
    }
    assert!(got_tool_use, "expected a ToolUse chunk");
}

#[tokio::test]
async fn test_openai_stream_chat_rate_limited() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "10")
                .set_body_string("rate limited"),
        )
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let result = provider.stream_chat(&test_chat_params()).await;
    match expect_err(result) {
        LoopalError::Provider(ProviderError::RateLimited { retry_after_ms }) => {
            assert_eq!(retry_after_ms, 10_000);
        }
        other => panic!("expected RateLimited error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_stream_chat_server_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_string("{\"error\":{\"message\":\"internal error\"}}"),
        )
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let result = provider.stream_chat(&test_chat_params()).await;
    match expect_err(result) {
        LoopalError::Provider(ProviderError::Api { status, message }) => {
            assert_eq!(status, 500);
            assert!(message.contains("internal error"));
        }
        other => panic!("expected Api error, got: {other:?}"),
    }
}
