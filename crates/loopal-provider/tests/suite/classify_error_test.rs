use loopal_error::{LoopalError, ProviderError};
use loopal_provider::{GoogleProvider, OpenAiCompatProvider, OpenAiProvider};
use loopal_provider_api::{ErrorClass, Provider, default_classify_error};

#[test]
fn default_classify_context_overflow_variant() {
    let err = LoopalError::Provider(ProviderError::ContextOverflow {
        message: "any".into(),
    });
    assert_eq!(default_classify_error(&err), ErrorClass::ContextOverflow);
}

#[test]
fn default_classify_rate_limited_is_retryable() {
    let err = LoopalError::Provider(ProviderError::RateLimited { retry_after_ms: 1 });
    assert_eq!(default_classify_error(&err), ErrorClass::Retryable);
}

#[test]
fn default_classify_500_is_retryable() {
    let err = LoopalError::Provider(ProviderError::Api {
        status: 500,
        message: "internal".into(),
    });
    assert_eq!(default_classify_error(&err), ErrorClass::Retryable);
}

#[test]
fn default_classify_http_transport_is_retryable() {
    let err = LoopalError::Provider(ProviderError::Http("connection reset".into()));
    assert_eq!(default_classify_error(&err), ErrorClass::Retryable);
}

#[test]
fn default_classify_400_generic_is_fatal() {
    // Without provider-specific keyword matching, 400 falls through to Fatal.
    let err = LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: "anything not matching keywords".into(),
    });
    assert_eq!(default_classify_error(&err), ErrorClass::Fatal);
}

#[test]
fn openai_classify_400_context_overflow_keywords() {
    let p = OpenAiProvider::new("k".into());
    for msg in [
        "This model's maximum context length is 128000 tokens",
        "context_length_exceeded",
        "input exceeds the maximum allowed",
        "too many tokens in request",
    ] {
        let err = LoopalError::Provider(ProviderError::Api {
            status: 400,
            message: msg.into(),
        });
        assert_eq!(
            p.classify_error(&err),
            ErrorClass::ContextOverflow,
            "msg={msg}"
        );
    }
}

#[test]
fn openai_classify_400_unrelated_is_fatal() {
    let p = OpenAiProvider::new("k".into());
    let err = LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: r#"{"error":{"type":"invalid_request_error"}}"#.into(),
    });
    assert_eq!(p.classify_error(&err), ErrorClass::Fatal);
}

#[test]
fn openai_classify_429_retryable() {
    let p = OpenAiProvider::new("k".into());
    let err = LoopalError::Provider(ProviderError::RateLimited {
        retry_after_ms: 1000,
    });
    assert_eq!(p.classify_error(&err), ErrorClass::Retryable);
}

#[test]
fn google_classify_400_context_overflow_keywords() {
    let p = GoogleProvider::new("k".into());
    for msg in [
        "Request token count exceeds the maximum",
        "input is too long for this model",
        "exceeds the maximum number of input tokens",
    ] {
        let err = LoopalError::Provider(ProviderError::Api {
            status: 400,
            message: msg.into(),
        });
        assert_eq!(
            p.classify_error(&err),
            ErrorClass::ContextOverflow,
            "msg={msg}"
        );
    }
}

#[test]
fn google_classify_500_retryable() {
    let p = GoogleProvider::new("k".into());
    let err = LoopalError::Provider(ProviderError::Api {
        status: 500,
        message: "internal".into(),
    });
    assert_eq!(p.classify_error(&err), ErrorClass::Retryable);
}

#[test]
fn openai_compat_classify_400_context_overflow_keywords() {
    let p = OpenAiCompatProvider::new("k".into(), "https://x".into(), "compat".into());
    for msg in [
        "context_length_exceeded",
        "prompt is too long for the model",
        "maximum context length 32k",
    ] {
        let err = LoopalError::Provider(ProviderError::Api {
            status: 400,
            message: msg.into(),
        });
        assert_eq!(
            p.classify_error(&err),
            ErrorClass::ContextOverflow,
            "msg={msg}"
        );
    }
}

#[test]
fn openai_compat_classify_invalid_request_fatal() {
    let p = OpenAiCompatProvider::new("k".into(), "https://x".into(), "compat".into());
    let err = LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: "tool_choice value is invalid".into(),
    });
    assert_eq!(p.classify_error(&err), ErrorClass::Fatal);
}
