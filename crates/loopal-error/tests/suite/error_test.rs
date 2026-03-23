use loopal_error::{ConfigError, HookError, LoopalError, ProviderError, StorageError, ToolError};

// --- ProviderError ---

#[test]
fn test_provider_error_is_rate_limited_true() {
    let err = ProviderError::RateLimited {
        retry_after_ms: 5000,
    };
    assert!(err.is_rate_limited());
}

#[test]
fn test_provider_error_is_rate_limited_false_for_http() {
    let err = ProviderError::Http("timeout".into());
    assert!(!err.is_rate_limited());
}

#[test]
fn test_provider_error_is_rate_limited_false_for_api() {
    let err = ProviderError::Api {
        status: 500,
        message: "internal".into(),
    };
    assert!(!err.is_rate_limited());
}

#[test]
fn test_provider_error_is_rate_limited_false_for_model_not_found() {
    let err = ProviderError::ModelNotFound("gpt-99".into());
    assert!(!err.is_rate_limited());
}

#[test]
fn test_provider_error_is_rate_limited_false_for_sse_parse() {
    let err = ProviderError::SseParse("bad data".into());
    assert!(!err.is_rate_limited());
}

#[test]
fn test_provider_error_is_rate_limited_false_for_stream_ended() {
    let err = ProviderError::StreamEnded;
    assert!(!err.is_rate_limited());
}

#[test]
fn test_provider_error_retry_after_ms_some() {
    let err = ProviderError::RateLimited {
        retry_after_ms: 3000,
    };
    assert_eq!(err.retry_after_ms(), Some(3000));
}

#[test]
fn test_provider_error_retry_after_ms_none_for_http() {
    let err = ProviderError::Http("err".into());
    assert_eq!(err.retry_after_ms(), None);
}

#[test]
fn test_provider_error_retry_after_ms_none_for_api() {
    let err = ProviderError::Api {
        status: 429,
        message: "too many".into(),
    };
    assert_eq!(err.retry_after_ms(), None);
}

// --- LoopalError ---

#[test]
fn test_loopal_error_is_rate_limited_true() {
    let err = LoopalError::Provider(ProviderError::RateLimited {
        retry_after_ms: 1000,
    });
    assert!(err.is_rate_limited());
}

#[test]
fn test_loopal_error_is_rate_limited_false_for_non_provider() {
    let err = LoopalError::Other("something".into());
    assert!(!err.is_rate_limited());
}

#[test]
fn test_loopal_error_is_rate_limited_false_for_non_rate_limit_provider() {
    let err = LoopalError::Provider(ProviderError::Http("err".into()));
    assert!(!err.is_rate_limited());
}

#[test]
fn test_loopal_error_retry_after_ms_some() {
    let err = LoopalError::Provider(ProviderError::RateLimited {
        retry_after_ms: 2000,
    });
    assert_eq!(err.retry_after_ms(), Some(2000));
}

#[test]
fn test_loopal_error_retry_after_ms_none() {
    let err = LoopalError::Permission("denied".into());
    assert_eq!(err.retry_after_ms(), None);
}

// --- Display implementations ---

#[test]
fn test_provider_error_display_http() {
    let err = ProviderError::Http("connection refused".into());
    assert_eq!(format!("{err}"), "HTTP error: connection refused");
}

#[test]
fn test_provider_error_display_sse_parse() {
    let err = ProviderError::SseParse("invalid json".into());
    assert_eq!(format!("{err}"), "SSE parse error: invalid json");
}

#[test]
fn test_provider_error_display_api() {
    let err = ProviderError::Api {
        status: 401,
        message: "unauthorized".into(),
    };
    assert_eq!(
        format!("{err}"),
        "API error: status=401, message=unauthorized"
    );
}

#[test]
fn test_provider_error_display_model_not_found() {
    let err = ProviderError::ModelNotFound("gpt-99".into());
    assert_eq!(format!("{err}"), "Model not found: gpt-99");
}

#[test]
fn test_provider_error_display_rate_limited() {
    let err = ProviderError::RateLimited {
        retry_after_ms: 5000,
    };
    assert_eq!(format!("{err}"), "Rate limited: retry after 5000ms");
}

#[test]
fn test_provider_error_display_stream_ended() {
    let err = ProviderError::StreamEnded;
    assert_eq!(format!("{err}"), "Stream ended unexpectedly");
}

#[test]
fn test_loopal_error_display_provider() {
    let err = LoopalError::Provider(ProviderError::StreamEnded);
    assert_eq!(
        format!("{err}"),
        "Provider error: Stream ended unexpectedly"
    );
}

#[test]
fn test_loopal_error_display_tool() {
    let err = LoopalError::Tool(ToolError::NotFound("foo".into()));
    assert_eq!(format!("{err}"), "Tool error: Tool not found: foo");
}

#[test]
fn test_loopal_error_display_config() {
    let err = LoopalError::Config(ConfigError::MissingField("model".into()));
    assert_eq!(
        format!("{err}"),
        "Config error: Missing required field: model"
    );
}

#[test]
fn test_loopal_error_display_storage() {
    let err = LoopalError::Storage(StorageError::SessionNotFound("abc".into()));
    assert_eq!(format!("{err}"), "Storage error: Session not found: abc");
}

#[test]
fn test_loopal_error_display_permission() {
    let err = LoopalError::Permission("not allowed".into());
    assert_eq!(format!("{err}"), "Permission denied: not allowed");
}

#[test]
fn test_loopal_error_display_hook() {
    let err = LoopalError::Hook(HookError::Rejected("blocked".into()));
    assert_eq!(format!("{err}"), "Hook error: Hook rejected: blocked");
}
