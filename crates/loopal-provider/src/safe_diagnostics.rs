use futures::StreamExt;
use loopal_error::ProviderError;

const MAX_API_ERROR_CHARS: usize = 2048;
const MAX_API_ERROR_BYTES: usize = 64 * 1024;

pub(crate) fn network_error(provider: &str, error: &reqwest::Error) -> ProviderError {
    let reason = if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_decode() {
        "response decode failed"
    } else {
        "request failed"
    };
    ProviderError::Http(format!("{provider} {reason}"))
}

pub(crate) fn api_error_message(provider: &str, body: &str, secrets: &[&str]) -> String {
    let lower = body.to_ascii_lowercase();
    if is_context_overflow(&lower) {
        return "maximum context length exceeded".into();
    }
    let contains_secret = secrets
        .iter()
        .any(|secret| !secret.is_empty() && body.contains(secret));
    let sensitive_shape = [
        "http://",
        "https://",
        "authorization",
        "api_key",
        "api-key",
        "api key",
        "apikey",
        "bearer ",
        "{{secret:",
        "token",
        "password",
        "cookie",
        "credential",
        "secret",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if contains_secret || sensitive_shape {
        return format!("{provider} API request failed");
    }
    let safe: String = body
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(MAX_API_ERROR_CHARS)
        .collect();
    if safe.trim().is_empty() {
        format!("{provider} API request failed")
    } else {
        safe
    }
}

pub(crate) async fn response_error_message(
    provider: &str,
    response: reqwest::Response,
    secrets: &[&str],
) -> String {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_API_ERROR_BYTES as u64)
    {
        return format!("{provider} API request failed");
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else {
            return format!("{provider} API request failed");
        };
        if bytes.len().saturating_add(chunk.len()) > MAX_API_ERROR_BYTES {
            return format!("{provider} API request failed");
        }
        bytes.extend_from_slice(&chunk);
    }
    api_error_message(provider, &String::from_utf8_lossy(&bytes), secrets)
}

fn is_context_overflow(message: &str) -> bool {
    message.contains("maximum context length")
        || message.contains("context_length_exceeded")
        || message.contains("exceeds the maximum")
        || message.contains("too many tokens")
        || message.contains("prompt is too long")
        || message.contains("input is too long")
        || message.contains("token count")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn network_error_never_exposes_request_url_or_credentials() {
        let marker = "provider-network-secret-marker";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_millis(250))
            .build()
            .unwrap()
            .get(format!("http://user:{marker}@{address}/path?key={marker}"))
            .send()
            .await
            .unwrap_err();
        assert!(error.url().is_some_and(|url| url.as_str().contains(marker)));
        let safe = network_error("google", &error);
        assert!(!format!("{safe}").contains(marker));
        assert!(!format!("{safe:?}").contains(marker));
    }

    #[test]
    fn api_error_redacts_secrets_urls_and_authorization_shapes() {
        let marker = "provider-api-secret-marker";
        let unsafe_body =
            format!("Authorization: Bearer {marker}; https://api.test/path?key={marker}");
        let safe = api_error_message("openai", &unsafe_body, &[marker]);
        assert_eq!(safe, "openai API request failed");
        assert!(!safe.contains(marker));
        let error = ProviderError::Api {
            status: 500,
            message: safe,
            retry_after_ms: None,
        };
        assert!(!format!("{error}").contains(marker));
        assert!(!format!("{error:?}").contains(marker));
        assert_eq!(
            api_error_message("openai", "internal error", &[marker]),
            "internal error"
        );
        assert_eq!(
            api_error_message("openai", &"x".repeat(5000), &[])
                .chars()
                .count(),
            MAX_API_ERROR_CHARS
        );
    }
}
