//! Drive the LLM call that produces a context-compaction summary.
//!
//! Tunables (max_tokens, retry schedule) live in `crate::compact_config`
//! so the call site here is purely mechanism: build prompt → stream →
//! retry on transient failures → propagate everything else.

use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};

use futures::StreamExt;

use super::compact_prompt::{SYSTEM_PROMPT, build_prompt};
use crate::compact_config::{COMPACT_MAX_OUTPUT_TOKENS, RETRY_BACKOFF};

pub(super) async fn call_summarization_llm(
    provider: &dyn Provider,
    model: &str,
    conversation_text: &str,
    custom_instructions: Option<&str>,
) -> Result<String, LoopalError> {
    let prompt = build_prompt(conversation_text, custom_instructions);

    let mut last_error: Option<LoopalError> = None;
    // First attempt has no delay; subsequent attempts use the backoff schedule.
    let no_delay = [std::time::Duration::ZERO];
    let delays = no_delay.iter().chain(RETRY_BACKOFF.iter());
    for (attempt, delay) in delays.enumerate() {
        if !delay.is_zero() {
            tokio::time::sleep(*delay).await;
        }
        match drive_once(provider, model, &prompt).await {
            Ok(text) => return Ok(text),
            Err(e) => {
                let retryable = is_retryable(&e);
                tracing::warn!(
                    attempt,
                    retryable,
                    error = %e,
                    "summarization LLM call failed"
                );
                if !retryable {
                    return Err(e);
                }
                last_error = Some(e);
            }
        }
    }
    Err(last_error.expect("loop ran at least once"))
}

async fn drive_once(
    provider: &dyn Provider,
    model: &str,
    prompt: &str,
) -> Result<String, LoopalError> {
    let params = ChatParams {
        model: model.to_string(),
        messages: vec![Message::user(prompt)],
        system_prompt: SYSTEM_PROMPT.to_string(),
        tools: vec![],
        max_tokens: COMPACT_MAX_OUTPUT_TOKENS,
        temperature: Some(0.0),
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    };

    let mut stream = provider.stream_chat(&params).await?;
    let mut raw = String::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(StreamChunk::Text { text }) => raw.push_str(&text),
            Ok(StreamChunk::Done { .. }) => break,
            Err(e) => return Err(e),
            _ => {}
        }
    }

    Ok(raw)
}

fn is_retryable(err: &LoopalError) -> bool {
    err.is_retryable() && !err.is_context_overflow()
}
