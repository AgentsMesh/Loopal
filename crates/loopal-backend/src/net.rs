//! Network operations with timeout and size limits.

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::FetchResult;

use crate::limits::ResourceLimits;

/// Fetch content from a URL with timeout, size limit, and domain check.
pub async fn fetch_url(
    url: &str,
    policy: Option<&ResolvedPolicy>,
    limits: &ResourceLimits,
) -> Result<FetchResult, ToolIoError> {
    // Validate scheme
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(ToolIoError::Network(format!(
            "invalid URL (must start with http:// or https://): {url}"
        )));
    }

    // Domain check
    if let Some(pol) = policy
        && let Some(domain) = loopal_sandbox::network::extract_domain(url)
        && let Err(reason) = loopal_sandbox::network::check_domain(&pol.network, &domain)
    {
        return Err(ToolIoError::PermissionDenied(reason));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(limits.fetch_timeout_secs))
        .build()
        .map_err(|e| ToolIoError::Network(e.to_string()))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ToolIoError::Network(format!("HTTP request failed: {e}")))?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(ToolIoError::Network(format!("HTTP {status}")));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Stream body with size limit
    use futures::StreamExt;
    let mut body_bytes = Vec::with_capacity(8192);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ToolIoError::Network(format!("read error: {e}")))?;
        body_bytes.extend_from_slice(&chunk);
        if body_bytes.len() >= limits.max_fetch_bytes {
            body_bytes.truncate(limits.max_fetch_bytes);
            break;
        }
    }

    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    Ok(FetchResult {
        body,
        content_type,
        status,
    })
}
