//! Convenience builders for `StreamChunk` sequences.
//!
//! Eliminates `Ok(StreamChunk::Text { text: "...".into() })` boilerplate.

use loopal_error::LoopalError;
use loopal_provider_api::{StopReason, StreamChunk};

pub fn text(s: &str) -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::Text {
        text: s.to_string(),
    })
}

pub fn tool_use(
    id: &str,
    name: &str,
    input: serde_json::Value,
) -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::ToolUse {
        id: id.to_string(),
        name: name.to_string(),
        input,
    })
}

pub fn usage(input_tokens: u32, output_tokens: u32) -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::Usage {
        input_tokens,
        output_tokens,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        thinking_tokens: 0,
    })
}

pub fn done() -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::Done {
        stop_reason: StopReason::EndTurn,
    })
}

/// A complete single-turn text response: text + usage + done.
pub fn text_turn(s: &str) -> Vec<Result<StreamChunk, LoopalError>> {
    vec![text(s), usage(5, 3), done()]
}

/// A tool-call turn: tool_use + usage + done.
pub fn tool_turn(
    id: &str,
    name: &str,
    input: serde_json::Value,
) -> Vec<Result<StreamChunk, LoopalError>> {
    vec![tool_use(id, name, input), usage(10, 5), done()]
}

/// A provider error (simulates LLM failure mid-stream).
pub fn provider_error(msg: &str) -> Result<StreamChunk, LoopalError> {
    Err(LoopalError::Provider(loopal_error::ProviderError::Http(
        msg.to_string(),
    )))
}

/// A rate-limit error from the provider.
pub fn rate_limited(retry_ms: u64) -> Result<StreamChunk, LoopalError> {
    Err(LoopalError::Provider(
        loopal_error::ProviderError::RateLimited {
            retry_after_ms: retry_ms,
        },
    ))
}

pub fn thinking(s: &str) -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::Thinking {
        text: s.to_string(),
    })
}

pub fn thinking_signature(sig: &str) -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::ThinkingSignature {
        signature: sig.to_string(),
    })
}

pub fn done_max_tokens() -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::Done {
        stop_reason: StopReason::MaxTokens,
    })
}
