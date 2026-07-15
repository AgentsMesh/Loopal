use super::stream_helpers::{collect_chunks, test_chat_params};
use loopal_provider::OpenAiProvider;
use loopal_provider_api::{Provider, StreamChunk};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_responses_api_incomplete_event() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
event: response.output_text.delta\n\
data: {\"type\":\"response.output_text.delta\",\"delta\":\"X\"}\n\n\
event: response.incomplete\n\
data: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"resp_1\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":{\"input_tokens\":7,\"output_tokens\":3,\"output_tokens_details\":{\"reasoning_tokens\":2}}}}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let ok_chunks: Vec<_> = chunks.iter().filter_map(|c| c.as_ref().ok()).collect();
    assert_eq!(ok_chunks.len(), 3);
    match ok_chunks[0] {
        StreamChunk::Text { text } => assert_eq!(text, "X"),
        other => panic!("expected Text, got: {other:?}"),
    }
    assert!(matches!(
        ok_chunks[1],
        StreamChunk::Usage {
            input_tokens: 7,
            output_tokens: 3,
            thinking_tokens: 2,
            ..
        }
    ));
    match ok_chunks[2] {
        StreamChunk::Done { stop_reason } => {
            assert_eq!(*stop_reason, loopal_provider_api::StopReason::MaxTokens);
        }
        other => panic!("expected Done with MaxTokens, got: {other:?}"),
    }
}

#[tokio::test]
async fn responses_api_rejects_unknown_incomplete_reason() {
    let mock_server = MockServer::start().await;
    let sse = "data: {\"type\":\"response.incomplete\",\"response\":{\"incomplete_details\":{\"reason\":\"content_filter\"}}}\n\n";
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&mock_server)
        .await;
    let provider = OpenAiProvider::new("test-key".into()).with_base_url(mock_server.uri());
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;
    assert!(chunks[0].is_err());
}
