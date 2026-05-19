use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};

use futures::StreamExt;
use tokio_util::sync::CancellationToken;

use super::compact_prompt::{SYSTEM_PROMPT, build_prompt};
use crate::compact_config::{COMPACT_MAX_OUTPUT_TOKENS, RETRY_BACKOFF};

const CANCELLED_MSG: &str = "compact cancelled by interrupt";

pub(super) async fn call_summarization_llm(
    provider: &dyn Provider,
    model: &str,
    conversation_text: &str,
    custom_instructions: Option<&str>,
    cancel: &CancellationToken,
) -> Result<String, LoopalError> {
    let prompt = build_prompt(conversation_text, custom_instructions);

    let mut last_error: Option<LoopalError> = None;
    let no_delay = [std::time::Duration::ZERO];
    let delays = no_delay.iter().chain(RETRY_BACKOFF.iter());
    for (attempt, delay) in delays.enumerate() {
        if !delay.is_zero() {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(LoopalError::Other(CANCELLED_MSG.into()));
                }
                _ = tokio::time::sleep(*delay) => {}
            }
        }
        match drive_once(provider, model, &prompt, cancel).await {
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
    cancel: &CancellationToken,
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

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                return Err(LoopalError::Other(CANCELLED_MSG.into()));
            }
            chunk = stream.next() => match chunk {
                Some(Ok(StreamChunk::Text { text })) => raw.push_str(&text),
                Some(Ok(StreamChunk::Done { .. })) => break,
                Some(Err(e)) => return Err(e),
                Some(_) => {}
                None => break,
            }
        }
    }

    Ok(raw)
}

fn is_retryable(err: &LoopalError) -> bool {
    err.is_retryable() && !err.is_context_overflow()
}
