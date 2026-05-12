//! Network operations with timeout and size limits.

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::FetchResult;
use loopal_tool_api::save_to_overflow_file;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue};

use crate::limits::ResourceLimits;

// reason: anti-bot systems (Baidu, Cloudflare, CDN edges) reject or downgrade
// responses for the default `reqwest/x.y.z` UA, returning JS-challenge stubs
// instead of real content. A browser-like UA + Accept headers is the baseline
// for "actually downloads the page" rather than a token success.
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36";

// reason: pages served as HTTP 200 but containing one of these markers are
// JS-challenge / anti-bot interstitials, not real content. Surfacing them as
// success would feed the LLM noise; treat as a typed network error instead.
const ANTI_BOT_MARKERS: &[&str] = &[
    "/httpservice/retry/enablejs",
    "cf-chl-bypass",
    "__cf_chl_jschl",
    "cf-mitigated",
    "cdn-cgi/challenge-platform",
    "just a moment...",
];

pub(crate) fn default_request_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    );
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7"),
    );
    headers
}

pub(crate) fn detect_anti_bot(body: &str) -> Option<&'static str> {
    let head: String = body
        .chars()
        .take(8192)
        .collect::<String>()
        .to_ascii_lowercase();
    ANTI_BOT_MARKERS.iter().copied().find(|m| head.contains(*m))
}

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
        .user_agent(BROWSER_USER_AGENT)
        .default_headers(default_request_headers())
        .timeout(limits.fetch_timeout)
        .build()
        .map_err(|e| ToolIoError::Network(e.to_string()))?;

    let original_host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned));

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ToolIoError::Network(format!("HTTP request failed: {e}")))?;

    let final_url_obj = response.url().clone();
    let final_host = final_url_obj.host_str().map(str::to_owned);
    let final_url = match (original_host, final_host) {
        (Some(orig), Some(fin)) if orig != fin => Some(final_url_obj.to_string()),
        _ => None,
    };

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
    let mut hit_limit = false;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ToolIoError::Network(format!("read error: {e}")))?;
        body_bytes.extend_from_slice(&chunk);
        if body_bytes.len() >= limits.max_fetch_bytes {
            body_bytes.truncate(limits.max_fetch_bytes);
            hit_limit = true;
            break;
        }
    }

    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    if let Some(marker) = detect_anti_bot(&body) {
        return Err(ToolIoError::Network(format!(
            "anti-bot challenge page returned (marker: {marker}); site requires JS or interactive challenge — try an MCP browser tool or different source"
        )));
    }
    let overflow_path = if hit_limit {
        Some(save_to_overflow_file(&body, "fetch_body"))
    } else {
        None
    };
    Ok(FetchResult {
        body,
        content_type,
        status,
        overflow_path,
        final_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // reason: every marker is load-bearing — losing detection of any one
    // (e.g. silently dropping `enablejs` when adapting to a new CDN signature)
    // lets the corresponding anti-bot stub flow back to the LLM as content.
    #[test]
    fn detect_anti_bot_recognizes_every_marker() {
        for marker in ANTI_BOT_MARKERS {
            let body = format!("<html>noise {marker} more noise</html>");
            assert_eq!(
                detect_anti_bot(&body),
                Some(*marker),
                "marker {marker} must be recognised"
            );
        }
    }

    #[test]
    fn detect_anti_bot_is_case_insensitive() {
        let body = "<html>JUST a Moment...</html>";
        assert!(
            detect_anti_bot(body).is_some(),
            "anti-bot marker must match regardless of case"
        );
    }

    #[test]
    fn detect_anti_bot_passes_clean_html_through() {
        let body = "<html><body><h1>Hello</h1><p>real content here</p></body></html>";
        assert_eq!(detect_anti_bot(body), None);
    }
}
