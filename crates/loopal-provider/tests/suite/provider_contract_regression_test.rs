use super::stream_helpers::{collect_chunks, test_chat_params};
use loopal_provider::{GoogleProvider, OpenAiCompatProvider, OpenAiProvider};
use loopal_provider_api::{ContentBlock, Message, MessageRole, Provider, StreamChunk};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn compat_tool_finish_emits_done_and_accepts_v1_base_url() {
    let server = MockServer::start().await;
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,",
        "\"id\":\"call_1\",\"function\":{\"name\":\"Read\",",
        "\"arguments\":\"{\\\"file_path\\\":\\\"README.md\\\"}\"}}]},",
        "\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;

    let provider = OpenAiCompatProvider::new(
        "key".into(),
        format!("{}/v1/", server.uri()),
        "compat".into(),
    );
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;
    assert!(
        matches!(chunks.first(), Some(Ok(StreamChunk::ToolUse { name, .. })) if name == "Read")
    );
    assert!(matches!(chunks.get(1), Some(Ok(StreamChunk::Done { .. }))));
}

#[tokio::test]
async fn openai_responses_accepts_v1_base_url() {
    let server = MockServer::start().await;
    let sse = concat!(
        "data: {\"type\":\"response.completed\",\"response\":{",
        "\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;

    let provider = OpenAiProvider::new("key".into()).with_base_url(format!("{}/v1", server.uri()));
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;
    assert!(
        chunks
            .iter()
            .any(|chunk| matches!(chunk, Ok(StreamChunk::Done { .. })))
    );
}

#[test]
fn google_tool_result_replays_function_name() {
    let provider = GoogleProvider::new("key".into());
    let messages = vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "README.md"}),
            }],
            origin: None,
            ephemeral_in_history: false,
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "fixture".into(),
                images: vec![],
                is_error: false,
                metadata: None,
            }],
            origin: None,
            ephemeral_in_history: false,
        },
    ];
    let contents = provider.build_contents_from_messages(&messages, &test_chat_params());
    assert_eq!(contents[1]["parts"][0]["functionResponse"]["name"], "Read");
}

#[test]
fn compat_replays_reasoning_content() {
    let provider = OpenAiCompatProvider::new("key".into(), "http://mock".into(), "compat".into());
    let messages = vec![Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Thinking {
            thinking: "retained reasoning".into(),
            signature: Some("sig".into()),
        }],
        origin: None,
        ephemeral_in_history: false,
    }];
    let wire = provider.build_messages_from_messages(&messages, &test_chat_params());
    assert_eq!(wire[1]["reasoning_content"], "retained reasoning");
}

#[test]
fn google_replays_thought_signature() {
    let provider = GoogleProvider::new("key".into());
    let messages = vec![Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Thinking {
            thinking: "retained thought".into(),
            signature: Some("sig-1".into()),
        }],
        origin: None,
        ephemeral_in_history: false,
    }];
    let wire = provider.build_contents_from_messages(&messages, &test_chat_params());
    assert_eq!(wire[0]["parts"][0]["thoughtSignature"], "sig-1");
}
