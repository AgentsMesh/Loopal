//! ACP extension notifications for Loopal-specific events.
//!
//! Events that don't map to standard ACP `SessionUpdate` variants are sent
//! as extension notifications with method names prefixed by `_loopal/`.

use serde_json::Value;

/// Build an ACP extension notification params value.
///
/// Extension methods are prefixed with `_` per ACP spec.
pub fn ext_notification(session_id: &str, ext_type: &str, data: Value) -> (String, Value) {
    let method = format!("_loopal/{ext_type}");
    let params = serde_json::json!({
        "sessionId": session_id,
        "data": data,
    });
    (method, params)
}

/// Build a `_loopal/retryError` notification.
pub fn retry_error(
    session_id: &str,
    message: &str,
    attempt: u32,
    max_attempts: u32,
) -> (String, Value) {
    ext_notification(
        session_id,
        "retryError",
        serde_json::json!({
            "message": message,
            "attempt": attempt,
            "maxAttempts": max_attempts,
        }),
    )
}

/// Build a `_loopal/tokenUsage` notification.
pub fn token_usage(session_id: &str, usage: &Value) -> (String, Value) {
    ext_notification(session_id, "tokenUsage", usage.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_error_format() {
        let (method, params) = retry_error("s1", "502 Bad Gateway", 2, 6);
        assert_eq!(method, "_loopal/retryError");
        assert_eq!(params["sessionId"], "s1");
        assert_eq!(params["data"]["attempt"], 2);
    }

    #[test]
    fn token_usage_format() {
        let usage = serde_json::json!({"inputTokens": 100, "outputTokens": 50});
        let (method, params) = token_usage("s1", &usage);
        assert_eq!(method, "_loopal/tokenUsage");
        assert_eq!(params["data"]["inputTokens"], 100);
    }
}
