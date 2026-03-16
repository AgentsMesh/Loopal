use futures::StreamExt;
use loopagent_provider::OpenAiProvider;
use loopagent_types::error::LoopAgentError;
use loopagent_types::message::Message;
use loopagent_types::provider::{ChatParams, Provider, StreamChunk};
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
    }
}

async fn collect_chunks(
    mut stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<StreamChunk, LoopAgentError>> + Send + Unpin>,
    >,
) -> Vec<Result<StreamChunk, LoopAgentError>> {
    let mut chunks = Vec::new();
    while let Some(item) = stream.next().await {
        chunks.push(item);
    }
    chunks
}

#[tokio::test]
async fn test_sse_done_sentinel_filtered() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
data: {\"choices\":[{\"delta\":{\"content\":\"X\"},\"finish_reason\":null}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
data: [DONE]\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(sse_body, "text/event-stream"),
        )
        .mount(&mock_server)
        .await;

    let provider =
        OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let ok_chunks: Vec<_> = chunks.iter().filter_map(|c| c.as_ref().ok()).collect();
    assert_eq!(ok_chunks.len(), 2);
    match ok_chunks[0] {
        StreamChunk::Text { text } => assert_eq!(text, "X"),
        other => panic!("expected Text, got: {:?}", other),
    }
    match ok_chunks[1] {
        StreamChunk::Done => {}
        other => panic!("expected Done, got: {:?}", other),
    }
}
