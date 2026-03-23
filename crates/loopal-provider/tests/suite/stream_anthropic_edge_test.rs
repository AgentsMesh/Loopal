use futures::StreamExt;
use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider::AnthropicProvider;
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
