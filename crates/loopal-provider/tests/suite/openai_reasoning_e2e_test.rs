use super::stream_helpers::{collect_chunks, test_chat_params};
use loopal_provider::OpenAiProvider;
use loopal_provider_api::{Provider, StreamChunk};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn reasoning_and_web_search_round_trip() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
event: response.reasoning_summary_text.delta\n\
data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"Searching for info\"}\n\n\
event: response.output_item.done\n\
data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"reasoning\",\"id\":\"rs_abc123\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"Searching for info\"}]}}\n\n\
event: response.output_item.done\n\
data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_def456\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"query\":\"rust async\"}}}\n\n\
event: response.output_text.delta\n\
data: {\"type\":\"response.output_text.delta\",\"delta\":\"Here is what I found.\"}\n\n\
event: response.completed\n\
data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"usage\":{\"input_tokens\":20,\"output_tokens\":10,\"total_tokens\":30}}}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    let mut got_thinking = false;
    let mut got_signature = false;
    let mut got_server_tool_use = false;
    let mut got_text = false;

    for chunk in chunks.into_iter().flatten() {
        match chunk {
            StreamChunk::Thinking { text } => {
                assert_eq!(text, "Searching for info");
                got_thinking = true;
            }
            StreamChunk::ThinkingSignature { signature } => {
                assert_eq!(signature, "rs_abc123");
                got_signature = true;
            }
            StreamChunk::ServerToolUse { id, name, input } => {
                assert_eq!(id, "ws_def456");
                assert_eq!(name, "web_search");
                assert_eq!(input["query"], "rust async");
                got_server_tool_use = true;
            }
            StreamChunk::Text { text } => {
                assert_eq!(text, "Here is what I found.");
                got_text = true;
            }
            _ => {}
        }
    }
    assert!(
        got_thinking,
        "expected Thinking chunk from reasoning_summary_text.delta"
    );
    assert!(
        got_signature,
        "expected ThinkingSignature from reasoning output_item.done"
    );
    assert!(
        got_server_tool_use,
        "expected ServerToolUse from web_search_call"
    );
    assert!(got_text, "expected Text chunk");
}

#[tokio::test]
async fn reasoning_id_missing_does_not_emit_signature() {
    let mock_server = MockServer::start().await;

    let sse_body = "\
event: response.output_item.done\n\
data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"reasoning\",\"id\":\"\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"thinking\"}]}}\n\n\
event: response.completed\n\
data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"usage\":{\"input_tokens\":5,\"output_tokens\":3,\"total_tokens\":8}}}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let provider = OpenAiProvider::new("test-key".to_string()).with_base_url(mock_server.uri());

    let stream = provider.stream_chat(&test_chat_params()).await.unwrap();
    let chunks = collect_chunks(stream).await;

    for chunk in chunks.into_iter().flatten() {
        assert!(
            !matches!(chunk, StreamChunk::ThinkingSignature { .. }),
            "empty reasoning ID should not emit ThinkingSignature"
        );
    }
}
