use futures::StreamExt;
use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider::OpenAiProvider;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_chat_params() -> ChatParams {
    ChatParams {
        model: "test-model".to_string(),
        messages: vec![Message::user("Hello")],
        system_prompt: "You are helpful.".to_string(),
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        thinking: None,
        debug_dump_dir: None,
    }
}

async fn collect_chunks(
    mut stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<StreamChunk, LoopalError>> + Send + Unpin>,
    >,
) -> Vec<Result<StreamChunk, LoopalError>> {
    let mut chunks = Vec::new();
    while let Some(item) = stream.next().await {
        chunks.push(item);
    }
    chunks
}

#[tokio::test]
async fn test_responses_api_incomplete_event() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
event: response.output_text.delta\n\
data: {\"type\":\"response.output_text.delta\",\"delta\":\"X\"}\n\n\
event: response.incomplete\n\
data: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"resp_1\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"}}}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let ok_chunks: Vec<_> = chunks.iter().filter_map(|c| c.as_ref().ok()).collect();
    assert_eq!(ok_chunks.len(), 2);
    match ok_chunks[0] {
        StreamChunk::Text { text } => assert_eq!(text, "X"),
        other => panic!("expected Text, got: {other:?}"),
    }
    match ok_chunks[1] {
        StreamChunk::Done { stop_reason } => {
            assert_eq!(*stop_reason, loopal_provider_api::StopReason::MaxTokens);
        }
        other => panic!("expected Done with MaxTokens, got: {other:?}"),
    }
}
