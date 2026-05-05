use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider::AnthropicProvider;
use loopal_provider_api::{ChatParams, ContinuationIntent, ContinuationReason, Provider};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SSE_END_TURN: &str = "\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n\
data: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n\
data: {\"type\":\"content_block_stop\"}\n\n\
data: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":0,\"output_tokens\":1}}\n\n\
data: {\"type\":\"message_stop\"}\n\n";

fn user(text: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

fn assistant(text: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

fn make_params(model: &str, messages: Vec<Message>) -> ChatParams {
    ChatParams {
        model: model.into(),
        messages,
        system_prompt: String::new(),
        tools: vec![],
        max_tokens: 16,
        temperature: None,
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

async fn captured_messages(server: &MockServer) -> Vec<serde_json::Value> {
    let reqs = server.received_requests().await.expect("requests");
    let body: serde_json::Value =
        serde_json::from_slice(&reqs.last().expect("at least one request").body)
            .expect("json body");
    body["messages"].as_array().expect("messages array").clone()
}

async fn install_ok_sse(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SSE_END_TURN, "text/event-stream"))
        .mount(server)
        .await;
}

#[tokio::test]
async fn anthropic_adaptive_model_appends_user_on_assistant_tail() {
    let server = MockServer::start().await;
    install_ok_sse(&server).await;
    let provider = AnthropicProvider::new("k".into()).with_base_url(server.uri());

    let params = make_params("claude-opus-4-7", vec![user("go"), assistant("partial")]);
    let _ = provider.stream_chat(&params).await.unwrap();

    let msgs = captured_messages(&server).await;
    let last = msgs.last().expect("non-empty");
    assert_eq!(
        last["role"], "user",
        "claude-opus-4-7 has supports_prefill=false → finalize_messages must append user tail"
    );
}

#[tokio::test]
async fn anthropic_haiku_does_not_append_user_when_assistant_tail_no_intent() {
    let server = MockServer::start().await;
    install_ok_sse(&server).await;
    let provider = AnthropicProvider::new("k".into()).with_base_url(server.uri());

    let params = make_params(
        "claude-haiku-3-5-20241022",
        vec![user("go"), assistant("partial")],
    );
    let _ = provider.stream_chat(&params).await.unwrap();

    let msgs = captured_messages(&server).await;
    let last = msgs.last().expect("non-empty");
    assert_eq!(
        last["role"], "assistant",
        "haiku has supports_prefill=true and no intent → no synthetic user appended"
    );
}

#[tokio::test]
async fn anthropic_haiku_appends_user_when_intent_set() {
    let server = MockServer::start().await;
    install_ok_sse(&server).await;
    let provider = AnthropicProvider::new("k".into()).with_base_url(server.uri());

    let mut params = make_params(
        "claude-haiku-3-5-20241022",
        vec![user("go"), assistant("partial")],
    );
    params.continuation_intent = Some(ContinuationIntent::AutoContinue {
        reason: ContinuationReason::PauseTurn,
    });
    let _ = provider.stream_chat(&params).await.unwrap();

    let msgs = captured_messages(&server).await;
    let last = msgs.last().expect("non-empty");
    assert_eq!(
        last["role"], "user",
        "intent.is_some() must trigger user tail injection regardless of supports_prefill"
    );
}

#[tokio::test]
async fn anthropic_existing_user_tail_unchanged() {
    let server = MockServer::start().await;
    install_ok_sse(&server).await;
    let provider = AnthropicProvider::new("k".into()).with_base_url(server.uri());

    let params = make_params(
        "claude-opus-4-7",
        vec![user("go"), assistant("partial"), user("more")],
    );
    let _ = provider.stream_chat(&params).await.unwrap();

    let msgs = captured_messages(&server).await;
    assert_eq!(msgs.len(), 3, "must not duplicate existing user tail");
    assert_eq!(msgs.last().unwrap()["role"], "user");
}
