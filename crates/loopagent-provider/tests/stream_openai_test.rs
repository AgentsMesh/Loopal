use futures::StreamExt;
use loopagent_provider::OpenAiProvider;
use loopagent_types::error::{LoopAgentError, ProviderError};
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
        debug_dump_dir: None,
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

fn expect_err(
    result: Result<loopagent_types::provider::ChatStream, LoopAgentError>,
) -> LoopAgentError {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err, got Ok"),
    }
}

#[tokio::test]
async fn test_openai_stream_chat_success() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5},\"choices\":[]}\n\n\
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
            Ok(StreamChunk::Usage { input_tokens, output_tokens, .. }) => {
                assert_eq!(*input_tokens, 10);
                assert_eq!(*output_tokens, 5);
                got_usage = true;
            }
            other => panic!("unexpected chunk: {:?}", other),
        }
    }
    assert!(got_text, "expected a Text chunk");
    assert!(got_done, "expected a Done chunk");
    assert!(got_usage, "expected a Usage chunk");
}

#[tokio::test]
async fn test_openai_stream_chat_tool_calls() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_abc\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"cmd\\\":\"}}]},\"finish_reason\":null}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"ls\\\"}\"}}]},\"finish_reason\":null}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n\
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

    let mut got_tool_use = false;
    for chunk in &chunks {
        if let Ok(StreamChunk::ToolUse { id, name, input }) = chunk {
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
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "10")
                .set_body_string("rate limited"),
        )
        .mount(&mock_server)
        .await;

    let provider =
        OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let result = provider.stream_chat(&test_chat_params()).await;
    match expect_err(result) {
        LoopAgentError::Provider(ProviderError::RateLimited { retry_after_ms }) => {
            assert_eq!(retry_after_ms, 10_000);
        }
        other => panic!("expected RateLimited error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_openai_stream_chat_server_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_string("{\"error\":{\"message\":\"internal error\"}}"),
        )
        .mount(&mock_server)
        .await;

    let provider =
        OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let result = provider.stream_chat(&test_chat_params()).await;
    match expect_err(result) {
        LoopAgentError::Provider(ProviderError::Api { status, message }) => {
            assert_eq!(status, 500);
            assert!(message.contains("internal error"));
        }
        other => panic!("expected Api error, got: {:?}", other),
    }
}

