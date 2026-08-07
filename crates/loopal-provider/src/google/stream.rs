use futures::stream::Stream;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{StopReason, StreamChunk};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct GoogleStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopalError>> + Send>>,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopalError>>,
}

impl Stream for GoogleStream {
    type Item = Result<StreamChunk, LoopalError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(item) = this.buffer.pop_front() {
            return Poll::Ready(Some(item));
        }

        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let chunks = parse_google_event(&data);
                let mut iter = chunks.into_iter();
                if let Some(first) = iter.next() {
                    this.buffer.extend(iter);
                    Poll::Ready(Some(first))
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

unsafe impl Send for GoogleStream {}
impl Unpin for GoogleStream {}

pub(crate) fn parse_google_event(data: &str) -> Vec<Result<StreamChunk, LoopalError>> {
    let parsed: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => {
            return vec![Err(ProviderError::SseParse(
                "invalid provider SSE JSON".into(),
            )
            .into())];
        }
    };

    let mut chunks = Vec::new();

    if let Some(usage) = parsed.get("usageMetadata") {
        let input = usage["promptTokenCount"].as_u64().unwrap_or(0) as u32;
        let output = usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32;
        let thinking = usage["thoughtsTokenCount"].as_u64().unwrap_or(0) as u32;
        if input > 0 || output > 0 {
            chunks.push(Ok(StreamChunk::Usage {
                input_tokens: input,
                output_tokens: output,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                thinking_tokens: thinking,
            }));
        }
    }

    if let Some(reason) = parsed["promptFeedback"]["blockReason"]
        .as_str()
        .filter(|reason| !reason.is_empty() && *reason != "BLOCK_REASON_UNSPECIFIED")
    {
        chunks.push(Err(google_terminal_error(
            "prompt blocked",
            reason,
            parsed["promptFeedback"]["blockReasonMessage"].as_str(),
        )));
        return chunks;
    }

    // Candidates
    let mut terminal: Option<Result<StreamChunk, LoopalError>> = None;
    if let Some(candidates) = parsed["candidates"].as_array() {
        for candidate in candidates {
            if let Some(parts) = candidate["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str()
                        && !text.is_empty()
                    {
                        if part
                            .get("thought")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            chunks.push(Ok(StreamChunk::Thinking {
                                text: text.to_string(),
                            }));
                        } else {
                            chunks.push(Ok(StreamChunk::Text {
                                text: text.to_string(),
                            }));
                        }
                    }

                    if let Some(signature) = part["thoughtSignature"].as_str() {
                        chunks.push(Ok(StreamChunk::ThinkingSignature {
                            signature: signature.to_string(),
                        }));
                    }

                    if let Some(fc) = part.get("functionCall") {
                        let name = fc["name"].as_str().unwrap_or("").to_string();
                        let args = fc.get("args").cloned().unwrap_or(json!({}));
                        chunks.push(Ok(StreamChunk::ToolUse {
                            id: format!("call_{}", uuid_v4_simple()),
                            name,
                            input: args,
                        }));
                    }
                }
            }

            // Grounding belongs to the completed candidate. Emit it before the
            // terminal marker because the runtime stops polling at `Done`.
            if let Some(meta) = candidate.get("groundingMetadata") {
                parse_grounding_metadata(meta, &mut chunks);
            }

            if let Some(candidate_terminal) = candidate_terminal(candidate) {
                let failed = candidate_terminal.is_err();
                terminal = Some(candidate_terminal);
                if failed {
                    // Any non-success finish reason fails the whole response.
                    // This also prevents an earlier candidate's `STOP` from
                    // masking a later blocked/error candidate.
                    break;
                }
            }
        }
    }

    if let Some(terminal) = terminal {
        chunks.push(terminal);
    }

    chunks
}

fn candidate_terminal(candidate: &Value) -> Option<Result<StreamChunk, LoopalError>> {
    let reason = candidate["finishReason"].as_str()?;
    match reason {
        "FINISH_REASON_UNSPECIFIED" | "" => None,
        "STOP" => Some(Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        })),
        "MAX_TOKENS" => Some(Ok(StreamChunk::Done {
            stop_reason: StopReason::MaxTokens,
        })),
        _ => Some(Err(google_terminal_error(
            "candidate terminated",
            reason,
            candidate["finishMessage"].as_str(),
        ))),
    }
}

fn google_terminal_error(scope: &str, reason: &str, provider_message: Option<&str>) -> LoopalError {
    let status = if reason == "SERVER_ERROR" { 500 } else { 400 };
    let safe_reason = crate::safe_diagnostics::api_error_message("google", reason, &[]);
    let detail = provider_message
        .map(|message| crate::safe_diagnostics::api_error_message("google", message, &[]))
        .filter(|message| !message.trim().is_empty());
    let message = match detail {
        Some(detail) => format!("google {scope}: {safe_reason}: {detail}"),
        None => format!("google {scope}: {safe_reason}"),
    };
    ProviderError::Api {
        status,
        message,
        retry_after_ms: None,
    }
    .into()
}

/// Simple collision-resistant ID generator (no uuid dep needed).
pub(crate) fn uuid_v4_simple() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}_{seq:x}")
}

/// Extract grounding metadata from Google Search Grounding and emit as server tool results.
fn parse_grounding_metadata(meta: &Value, chunks: &mut Vec<Result<StreamChunk, LoopalError>>) {
    let Some(grounding_chunks) = meta["groundingChunks"].as_array() else {
        return;
    };
    if grounding_chunks.is_empty() {
        return;
    }

    // Emit a ServerToolUse to indicate search was performed
    let search_id = format!("gs_{}", uuid_v4_simple());
    chunks.push(Ok(StreamChunk::ServerToolUse {
        id: search_id.clone(),
        name: "google_search".to_string(),
        input: json!({}),
    }));

    // Format sources into a structured result
    let sources: Vec<Value> = grounding_chunks
        .iter()
        .filter_map(|chunk| {
            let web = chunk.get("web")?;
            Some(json!({
                "url": web.get("uri").and_then(|v| v.as_str()).unwrap_or(""),
                "title": web.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            }))
        })
        .collect();

    chunks.push(Ok(StreamChunk::ServerToolResult {
        block_type: "web_search_tool_result".to_string(),
        tool_use_id: search_id,
        content: json!(sources),
    }));
}

#[cfg(test)]
mod terminal_tests {
    use super::*;

    fn terminal(data: Value) -> Vec<Result<StreamChunk, LoopalError>> {
        parse_google_event(&data.to_string())
    }

    #[test]
    fn only_stop_and_max_tokens_are_successful_terminal_reasons() {
        let stop = terminal(json!({"candidates": [{"finishReason": "STOP"}]}));
        assert!(matches!(
            stop.as_slice(),
            [Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn
            })]
        ));

        let max_tokens = terminal(json!({"candidates": [{"finishReason": "MAX_TOKENS"}]}));
        assert!(matches!(
            max_tokens.as_slice(),
            [Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens
            })]
        ));

        let unspecified = terminal(json!({
            "candidates": [{"finishReason": "FINISH_REASON_UNSPECIFIED"}]
        }));
        assert!(unspecified.is_empty());
    }

    #[test]
    fn blocked_and_invalid_candidate_reasons_fail_closed() {
        for reason in [
            "SAFETY",
            "RECITATION",
            "BLOCKLIST",
            "PROHIBITED_CONTENT",
            "SPII",
            "MALFORMED_FUNCTION_CALL",
            "UNEXPECTED_TOOL_CALL",
            "TOO_MANY_TOOL_CALLS",
            "OTHER",
            "NEW_REASON_FROM_PROVIDER",
        ] {
            let chunks = terminal(json!({
                "candidates": [{
                    "finishReason": reason,
                    "finishMessage": "provider rejected this response"
                }]
            }));
            assert_eq!(chunks.len(), 1, "reason={reason}");
            let error = chunks.into_iter().next().unwrap().unwrap_err();
            assert!(!error.is_retryable(), "reason={reason}, error={error}");
            assert!(matches!(
                error,
                LoopalError::Provider(ProviderError::Api { status: 400, .. })
            ));
        }
    }

    #[test]
    fn prompt_block_is_an_error_and_sensitive_provider_detail_is_redacted() {
        let chunks = terminal(json!({
            "promptFeedback": {
                "blockReason": "SAFETY",
                "blockReasonMessage": "Authorization: Bearer provider-secret"
            }
        }));
        let error = chunks.into_iter().next().unwrap().unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("SAFETY"));
        assert!(!rendered.contains("provider-secret"));
        assert!(matches!(
            error,
            LoopalError::Provider(ProviderError::Api { status: 400, .. })
        ));

        let chunks = terminal(json!({
            "promptFeedback": {
                "blockReason": "Authorization: Bearer reason-secret"
            }
        }));
        let rendered = chunks.into_iter().next().unwrap().unwrap_err().to_string();
        assert!(!rendered.contains("reason-secret"));
    }

    #[test]
    fn server_error_is_retryable_and_preserves_safe_reason_and_message() {
        let chunks = terminal(json!({
            "candidates": [{
                "finishReason": "SERVER_ERROR",
                "finishMessage": "backend temporarily unavailable"
            }]
        }));
        let error = chunks.into_iter().next().unwrap().unwrap_err();
        assert!(error.is_retryable());
        assert!(error.to_string().contains("SERVER_ERROR"));
        assert!(
            error
                .to_string()
                .contains("backend temporarily unavailable")
        );
        assert!(matches!(
            error,
            LoopalError::Provider(ProviderError::Api { status: 500, .. })
        ));
    }

    #[test]
    fn grounding_blocks_are_emitted_before_done() {
        let chunks = terminal(json!({
            "candidates": [{
                "finishReason": "STOP",
                "groundingMetadata": {
                    "groundingChunks": [{"web": {"uri": "https://example.test", "title": "x"}}]
                }
            }]
        }));
        assert!(matches!(
            chunks.first(),
            Some(Ok(StreamChunk::ServerToolUse { .. }))
        ));
        assert!(matches!(
            chunks.get(1),
            Some(Ok(StreamChunk::ServerToolResult { .. }))
        ));
        assert!(matches!(chunks.get(2), Some(Ok(StreamChunk::Done { .. }))));
    }
}
