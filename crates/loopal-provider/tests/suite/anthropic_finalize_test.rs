use loopal_error::{LoopalError, ProviderError};
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider::AnthropicProvider;
use loopal_provider_api::{
    ChatParams, ContinuationIntent, ContinuationReason, ErrorClass, Provider,
};

fn make_params(model: &str, messages: Vec<Message>) -> ChatParams {
    ChatParams {
        model: model.into(),
        messages,
        system_prompt: String::new(),
        tools: vec![],
        max_tokens: 4096,
        temperature: None,
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

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

fn provider() -> AnthropicProvider {
    AnthropicProvider::new("test-key".into())
}

fn last_role(messages: &[Message]) -> MessageRole {
    messages.last().expect("non-empty").role
}

fn last_text(messages: &[Message]) -> String {
    let last = messages.last().expect("non-empty");
    last.content
        .iter()
        .find_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn passthrough_when_supports_prefill_and_no_intent() {
    let p = provider();
    let params = make_params(
        "claude-haiku-3-5-20241022",
        vec![user("hi"), assistant("hello")],
    );
    let out = p.finalize_messages(&params);
    assert_eq!(last_role(&out), MessageRole::Assistant);
    assert_eq!(out.len(), 2);
}

#[test]
fn appends_user_when_supports_prefill_false_and_assistant_tail() {
    let p = provider();
    let params = make_params("claude-opus-4-7", vec![user("go"), assistant("partial")]);
    let out = p.finalize_messages(&params);
    assert_eq!(last_role(&out), MessageRole::User);
    assert!(last_text(&out).contains("[Continue from where you left off]"));
}

#[test]
fn no_append_when_supports_prefill_false_but_already_user_tail() {
    let p = provider();
    let params = make_params(
        "claude-opus-4-7",
        vec![user("go"), assistant("partial"), user("more")],
    );
    let out = p.finalize_messages(&params);
    assert_eq!(out.len(), 3);
    assert_eq!(last_text(&out), "more");
}

#[test]
fn appends_user_when_intent_set_even_if_supports_prefill() {
    let p = provider();
    let mut params = make_params(
        "claude-haiku-3-5-20241022",
        vec![user("go"), assistant("partial")],
    );
    params.continuation_intent = Some(ContinuationIntent::AutoContinue {
        reason: ContinuationReason::MaxTokensWithoutTools,
    });
    let out = p.finalize_messages(&params);
    assert_eq!(last_role(&out), MessageRole::User);
}

#[test]
fn invariant_holds_for_all_anthropic_thinking_models() {
    let p = provider();
    let models = [
        "claude-sonnet-4-20250514",
        "claude-opus-4-20250514",
        "claude-sonnet-4-6",
        "claude-opus-4-6",
        "claude-opus-4-7",
    ];
    let intents = [
        None,
        Some(ContinuationIntent::AutoContinue {
            reason: ContinuationReason::PauseTurn,
        }),
        Some(ContinuationIntent::AutoContinue {
            reason: ContinuationReason::StreamTruncated,
        }),
    ];
    for model in &models {
        for intent in &intents {
            let mut params = make_params(model, vec![user("a"), assistant("b")]);
            params.continuation_intent = intent.clone();
            let out = p.finalize_messages(&params);
            assert_eq!(
                last_role(&out),
                MessageRole::User,
                "model={} intent={:?}",
                model,
                intent
            );
        }
    }
}

#[test]
fn classify_prefill_rejection() {
    let p = provider();
    let err = LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: "This model does not support assistant message prefill. \
                  The conversation must end with a user message."
            .into(),
    });
    assert_eq!(p.classify_error(&err), ErrorClass::PrefillRejected);
}

#[test]
fn classify_server_block_error() {
    let p = provider();
    let err = LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: "code_execution server block without a corresponding tool_result".into(),
    });
    assert_eq!(p.classify_error(&err), ErrorClass::ServerBlockError);
}

#[test]
fn classify_context_overflow() {
    let p = provider();
    let err = LoopalError::Provider(ProviderError::ContextOverflow {
        message: "prompt is too long".into(),
    });
    assert_eq!(p.classify_error(&err), ErrorClass::ContextOverflow);
}

#[test]
fn classify_rate_limited_as_retryable() {
    let p = provider();
    let err = LoopalError::Provider(ProviderError::RateLimited {
        retry_after_ms: 30_000,
    });
    assert_eq!(p.classify_error(&err), ErrorClass::Retryable);
}

#[test]
fn classify_generic_400_as_fatal() {
    let p = provider();
    let err = LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: r#"{"error":{"type":"invalid_request_error"}}"#.into(),
    });
    assert_eq!(p.classify_error(&err), ErrorClass::Fatal);
}
