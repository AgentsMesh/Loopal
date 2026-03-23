use loopal_message::Message;
use loopal_provider_api::MiddlewareContext;

#[test]
fn test_middleware_context_construction_without_summarization_provider() {
    let ctx = MiddlewareContext {
        messages: vec![Message::user("hello")],
        system_prompt: "You are helpful.".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        total_input_tokens: 100,
        total_output_tokens: 50,
        total_cost: 0.001,
        max_context_tokens: 200_000,
        compact_model: None,
        summarization_provider: None,
    };

    assert_eq!(ctx.messages.len(), 1);
    assert_eq!(ctx.system_prompt, "You are helpful.");
    assert_eq!(ctx.model, "claude-sonnet-4-20250514");
    assert_eq!(ctx.total_input_tokens, 100);
    assert_eq!(ctx.total_output_tokens, 50);
    assert!((ctx.total_cost - 0.001).abs() < f64::EPSILON);
    assert_eq!(ctx.max_context_tokens, 200_000);
    assert!(ctx.summarization_provider.is_none());
}

#[test]
fn test_middleware_context_with_multiple_messages() {
    let ctx = MiddlewareContext {
        messages: vec![
            Message::user("hello"),
            Message::assistant("hi there"),
            Message::user("how are you?"),
        ],
        system_prompt: String::new(),
        model: "gpt-4".to_string(),
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost: 0.0,
        max_context_tokens: 128_000,
        compact_model: None,
        summarization_provider: None,
    };

    assert_eq!(ctx.messages.len(), 3);
    assert_eq!(ctx.max_context_tokens, 128_000);
}

#[test]
fn test_middleware_context_empty_messages() {
    let ctx = MiddlewareContext {
        messages: vec![],
        system_prompt: "system".to_string(),
        model: "model".to_string(),
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost: 0.0,
        max_context_tokens: 100_000,
        compact_model: None,
        summarization_provider: None,
    };

    assert!(ctx.messages.is_empty());
}
