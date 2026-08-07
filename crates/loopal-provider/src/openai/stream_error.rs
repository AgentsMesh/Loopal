use loopal_error::{LoopalError, ProviderError};
use serde_json::Value;

pub(super) fn response_failed(event: &Value) -> LoopalError {
    let error = event
        .pointer("/response/error")
        .or_else(|| event.get("error"));
    let code = field(error, "code");
    let kind = field(error, "type");
    let message = field(error, "message").to_ascii_lowercase();

    if code == "rate_limit_exceeded" || kind == "rate_limit_error" {
        return ProviderError::RateLimited {
            retry_after_ms: 30_000,
        }
        .into();
    }
    if matches!(
        code,
        "context_length_exceeded" | "max_context_length_exceeded"
    ) || message.contains("maximum context length")
        || message.contains("context length exceeded")
    {
        return ProviderError::ContextOverflow {
            message: "maximum context length exceeded".into(),
        }
        .into();
    }
    let status = if matches!(code, "server_error" | "internal_error") || kind == "server_error" {
        500
    } else {
        400
    };
    ProviderError::Api {
        status,
        message: "openai API request failed".into(),
        retry_after_ms: None,
    }
    .into()
}

fn field<'a>(error: Option<&'a Value>, name: &str) -> &'a str {
    error
        .and_then(|value| value.get(name))
        .and_then(Value::as_str)
        .unwrap_or("")
}
