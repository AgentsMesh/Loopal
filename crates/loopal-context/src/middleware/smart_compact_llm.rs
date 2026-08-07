use std::time::Duration;

use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, Provider, StreamChunk};

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use super::compact_prompt::{SYSTEM_PROMPT, build_prompt};
use super::compact_retry::{CompactRetryEvent, CompactRetryObserver};
use crate::compact_config::{COMPACT_MAX_OUTPUT_TOKENS, RETRY_BACKOFF};

const CANCELLED_MSG: &str = "compact cancelled by interrupt";
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

pub(super) async fn call_summarization_llm(
    provider: &dyn Provider,
    model: &str,
    conversation_text: &str,
    custom_instructions: Option<&str>,
    cancel: &CancellationToken,
    retry_observer: &dyn CompactRetryObserver,
) -> Result<String, LoopalError> {
    let prompt = build_prompt(conversation_text, custom_instructions);

    let mut attempt = 0usize;
    loop {
        let attempt_span = tracing::info_span!(
            "provider_attempt",
            loopal.provider.phase = "compaction",
            loopal.retry.attempt = attempt as u32 + 1,
            loopal.retry.count = attempt as u32,
            loopal.retry.max_retries = RETRY_BACKOFF.len() as u32,
            gen_ai.system = provider.name(),
            gen_ai.request.model = model,
            error.type = tracing::field::Empty,
            otel.status_code = tracing::field::Empty,
            otel.status_message = tracing::field::Empty,
        );
        let attempt_result = drive_once(provider, model, &prompt, cancel)
            .instrument(attempt_span.clone())
            .await;
        if let Err(error) = &attempt_result {
            attempt_span.record(
                "error.type",
                if cancel.is_cancelled() {
                    "cancelled"
                } else {
                    compact_attempt_error_type(error)
                },
            );
            attempt_span.record("otel.status_code", "ERROR");
            let message = if cancel.is_cancelled() {
                "provider attempt cancelled".to_string()
            } else {
                error.to_string()
            };
            attempt_span.record("otel.status_message", message.as_str());
        }
        drop(attempt_span);
        match attempt_result {
            Ok(text) => {
                if attempt > 0 {
                    retry_observer
                        .observe(CompactRetryEvent::Succeeded {
                            retries: attempt as u32,
                        })
                        .await;
                }
                return Ok(text);
            }
            Err(e) => {
                let retryable = is_retryable(&e);
                tracing::warn!(
                    attempt,
                    retryable,
                    error = %e,
                    "summarization LLM call failed"
                );
                if cancel.is_cancelled() {
                    retry_observer
                        .observe(CompactRetryEvent::Cancelled {
                            retries: attempt as u32,
                        })
                        .await;
                    return Err(e);
                }
                if !retryable {
                    if attempt > 0 {
                        retry_observer
                            .observe(CompactRetryEvent::Failed {
                                retries: attempt as u32,
                            })
                            .await;
                    }
                    return Err(e);
                }
                let Some(default_wait) = RETRY_BACKOFF.get(attempt) else {
                    retry_observer
                        .observe(CompactRetryEvent::Exhausted {
                            retries: attempt as u32,
                        })
                        .await;
                    return Err(e);
                };
                let wait = e
                    .retry_after_ms()
                    .map(Duration::from_millis)
                    .unwrap_or(*default_wait);
                retry_observer
                    .observe(CompactRetryEvent::Scheduled {
                        error: e.to_string(),
                        attempt: attempt as u32 + 1,
                        max_retries: RETRY_BACKOFF.len() as u32,
                        wait,
                    })
                    .await;
                attempt += 1;
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        retry_observer
                            .observe(CompactRetryEvent::Cancelled {
                                retries: attempt as u32,
                            })
                            .await;
                        return Err(LoopalError::Other(CANCELLED_MSG.into()));
                    }
                    _ = tokio::time::sleep(wait) => {}
                }
            }
        }
    }
}

async fn drive_once(
    provider: &dyn Provider,
    model: &str,
    prompt: &str,
    cancel: &CancellationToken,
) -> Result<String, LoopalError> {
    let params = ChatParams {
        model: model.to_string(),
        turns: vec![loopal_turn::Turn::single_user_prompt(prompt)],
        system_prompt: SYSTEM_PROMPT.to_string(),
        tools: vec![],
        max_tokens: COMPACT_MAX_OUTPUT_TOKENS,
        temperature: Some(0.0),
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    };

    let mut stream = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            return Err(LoopalError::Other(CANCELLED_MSG.into()));
        }
        result = provider.stream_chat(&params) => result?,
    };
    let mut raw = String::new();

    loop {
        let polled = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                return Err(LoopalError::Other(CANCELLED_MSG.into()));
            }
            polled = tokio::time::timeout(STREAM_IDLE_TIMEOUT, stream.next()) => polled,
        };
        match polled {
            Ok(Some(Ok(StreamChunk::Text { text }))) => raw.push_str(&text),
            Ok(Some(Ok(StreamChunk::Done { .. }))) => return Ok(raw),
            Ok(Some(Err(e))) => return Err(e),
            Ok(Some(_)) => {}
            Ok(None) => return Err(ProviderError::StreamEnded.into()),
            Err(_elapsed) => {
                return Err(ProviderError::Http(format!(
                    "provider stream idle for {}s during compaction",
                    STREAM_IDLE_TIMEOUT.as_secs()
                ))
                .into());
            }
        }
    }
}

fn is_retryable(err: &LoopalError) -> bool {
    err.is_retryable() && !err.is_context_overflow()
}

fn compact_attempt_error_type(error: &LoopalError) -> &'static str {
    match error {
        LoopalError::Provider(ProviderError::StreamEnded) => "stream_eof",
        LoopalError::Provider(ProviderError::Http(message)) if message.contains("stream idle") => {
            "stream_timeout"
        }
        _ => "provider",
    }
}
