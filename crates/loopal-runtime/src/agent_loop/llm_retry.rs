use std::time::Duration;

use futures::StreamExt;
use loopal_error::{LoopalError, ProviderError, Result};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ChatParams, Provider};
use tracing::{Instrument, Span, info, warn};

use super::cancel::TurnCancel;
use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;

const MAX_RETRIES: u32 = 6;
const BASE_WAIT_MS: u64 = 2000;
/// Max silence between stream chunks before the attempt is classified as a
/// transport failure rather than waiting for the provider's 300s request cap.
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Copy)]
enum AttemptFailureKind {
    Handshake,
    Stream,
    Eof,
    Idle,
}

impl AttemptFailureKind {
    fn telemetry_type(self) -> &'static str {
        match self {
            Self::Handshake => "provider",
            Self::Stream => "stream",
            Self::Eof => "stream_eof",
            Self::Idle => "stream_timeout",
        }
    }
}

enum ProviderAttemptOutcome {
    Complete(LlmStreamResult),
    Failed {
        error: LoopalError,
        result: LlmStreamResult,
        kind: AttemptFailureKind,
    },
    Cancelled(LlmStreamResult),
}

impl AgentLoopRunner {
    /// Run complete provider attempts under one retry budget.
    ///
    /// An attempt owns both the HTTP/SSE handshake and the stream terminal
    /// marker. A failure before semantic output is safe to replay. Once text,
    /// reasoning, or a tool call has escaped, replay would duplicate output, so
    /// the partial response is returned through the continuation path instead.
    /// Exposed for integration testing. Production callers use
    /// `stream_llm_with`.
    pub async fn retry_stream_response(
        &mut self,
        params: &ChatParams,
        provider: &dyn Provider,
        cancel: &TurnCancel,
    ) -> Result<LlmStreamResult> {
        let mut retry_count = 0;
        loop {
            if cancel.is_cancelled() {
                // Cancellation may race with the retry sleep completing. In
                // that case the loop reaches this top-of-attempt check with a
                // visible retry banner that still needs its terminal event.
                if retry_count > 0 {
                    self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                }
                return Ok(cancelled_result());
            }
            let attempt_span = tracing::info_span!(
                "provider_attempt",
                loopal.provider.phase = "main",
                loopal.retry.attempt = retry_count + 1,
                loopal.retry.count = retry_count,
                loopal.retry.max_retries = MAX_RETRIES,
                gen_ai.system = provider.name(),
                gen_ai.request.model = %params.model,
                error.type = tracing::field::Empty,
                otel.status_code = tracing::field::Empty,
                otel.status_message = tracing::field::Empty,
            );
            let outcome = self
                .run_provider_attempt(params, provider, cancel)
                .instrument(attempt_span.clone())
                .await?;
            match outcome {
                ProviderAttemptOutcome::Complete(result) => {
                    drop(attempt_span);
                    if retry_count > 0 {
                        self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                    }
                    return Ok(result);
                }
                ProviderAttemptOutcome::Cancelled(mut result) => {
                    record_attempt_cancelled(&attempt_span);
                    drop(attempt_span);
                    if retry_count > 0 {
                        self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                    }
                    result.stream_error = true;
                    return Ok(result);
                }
                ProviderAttemptOutcome::Failed {
                    error,
                    mut result,
                    kind,
                } => {
                    record_attempt_failure(&attempt_span, kind, &error);
                    drop(attempt_span);

                    let replay_safe = !result.has_semantic_output();
                    if replay_safe && error.is_retryable() && retry_count < MAX_RETRIES {
                        retry_count += 1;
                        self.schedule_retry(&error, retry_count, cancel).await?;
                        if cancel.is_cancelled() {
                            self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                            return Ok(cancelled_result());
                        }
                        continue;
                    }

                    // Every path out of an active retry lifecycle has exactly
                    // one terminal clear, including exhaustion and partial
                    // output that must switch to continuation.
                    if retry_count > 0 {
                        self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                    }

                    if replay_safe {
                        return Err(error);
                    }

                    warn!(
                        error = %error,
                        failure = kind.telemetry_type(),
                        "provider stream failed after semantic output"
                    );
                    self.emit_in_turn(AgentEventPayload::ProviderWarning {
                        message: partial_failure_message(kind, &error),
                    })
                    .await?;
                    if error.is_retryable() || is_transport_truncation(&error) {
                        result.stream_error = true;
                        result.stream_failure = Some(error);
                    } else {
                        result.terminal_error = Some(error);
                    }
                    return Ok(result);
                }
            }
        }
    }

    async fn schedule_retry(
        &self,
        error: &LoopalError,
        retry_count: u32,
        cancel: &TurnCancel,
    ) -> Result<()> {
        // reason: honor an explicit Retry-After as the server directs; add ±20%
        // jitter only to the default exponential backoff so many agents retrying
        // after a shared outage don't reconverge into a synchronized storm.
        let wait_ms = retry_wait_ms(retry_count, error.retry_after_ms(), retry_entropy());
        warn!(
            retry = retry_count,
            max_retries = MAX_RETRIES,
            wait_ms, error = %error, "retrying provider attempt"
        );
        self.emit_in_turn(AgentEventPayload::RetryError {
            message: format!("{}. Retrying in {:.1}s", error, wait_ms as f64 / 1000.0,),
            attempt: retry_count,
            max_attempts: MAX_RETRIES,
        })
        .await?;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(wait_ms)) => {}
            _ = cancel.cancelled() => {
                info!("cancelled during retry wait");
            }
        }
        Ok(())
    }

    async fn run_provider_attempt(
        &mut self,
        params: &ChatParams,
        provider: &dyn Provider,
        cancel: &TurnCancel,
    ) -> Result<ProviderAttemptOutcome> {
        let stream_result = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Ok(ProviderAttemptOutcome::Cancelled(cancelled_result())),
            result = provider.stream_chat(params) => result,
        };
        let mut stream = match stream_result {
            Ok(stream) => stream,
            Err(error) => {
                return Ok(ProviderAttemptOutcome::Failed {
                    error,
                    result: LlmStreamResult::default(),
                    kind: AttemptFailureKind::Handshake,
                });
            }
        };

        let mut result = LlmStreamResult::default();
        let mut received_done = false;
        loop {
            let polled = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Ok(ProviderAttemptOutcome::Cancelled(result));
                }
                polled = tokio::time::timeout(STREAM_IDLE_TIMEOUT, stream.next()) => polled,
            };
            match polled {
                Ok(Some(Ok(chunk))) => {
                    if !self
                        .handle_stream_chunk(chunk, &mut result, &mut received_done)
                        .await?
                    {
                        debug_assert!(received_done);
                        return Ok(ProviderAttemptOutcome::Complete(result));
                    }
                }
                Ok(Some(Err(error))) => {
                    return Ok(ProviderAttemptOutcome::Failed {
                        error,
                        result,
                        kind: AttemptFailureKind::Stream,
                    });
                }
                Ok(None) => {
                    return Ok(ProviderAttemptOutcome::Failed {
                        error: ProviderError::StreamEnded.into(),
                        result,
                        kind: AttemptFailureKind::Eof,
                    });
                }
                Err(_elapsed) => {
                    return Ok(ProviderAttemptOutcome::Failed {
                        error: ProviderError::Http(format!(
                            "provider stream idle for {}s",
                            STREAM_IDLE_TIMEOUT.as_secs()
                        ))
                        .into(),
                        result,
                        kind: AttemptFailureKind::Idle,
                    });
                }
            }
        }
    }

    /// Emit ThinkingComplete if thinking content or tokens were received.
    pub(super) async fn emit_thinking_complete(&self, result: &LlmStreamResult) -> Result<()> {
        let Some(token_count) = result.thinking_completion_tokens() else {
            return Ok(());
        };
        self.emit_in_turn(AgentEventPayload::ThinkingComplete { token_count })
            .await
    }
}

fn cancelled_result() -> LlmStreamResult {
    LlmStreamResult {
        stream_error: true,
        ..Default::default()
    }
}

fn is_transport_truncation(error: &LoopalError) -> bool {
    matches!(
        error,
        LoopalError::Provider(
            ProviderError::Http(_) | ProviderError::SseParse(_) | ProviderError::StreamEnded
        )
    )
}

fn partial_failure_message(kind: AttemptFailureKind, error: &LoopalError) -> String {
    match kind {
        AttemptFailureKind::Eof => {
            "Response stream ended unexpectedly - possible network interruption".into()
        }
        AttemptFailureKind::Idle => {
            "Response stream stalled (no data received) - treating as interruption".into()
        }
        AttemptFailureKind::Handshake | AttemptFailureKind::Stream => error.to_string(),
    }
}

fn record_attempt_failure(span: &Span, kind: AttemptFailureKind, error: &LoopalError) {
    span.record("error.type", kind.telemetry_type());
    span.record("otel.status_code", "ERROR");
    let message = error.to_string();
    span.record("otel.status_message", message.as_str());
}

fn record_attempt_cancelled(span: &Span) {
    span.record("error.type", "cancelled");
    span.record("otel.status_code", "ERROR");
    span.record("otel.status_message", "provider attempt cancelled");
}

/// Calculate the delay before a one-based retry number.
///
/// Retry-After is already the server's delay for the current response and must
/// not be multiplied by the client's exponential backoff.
fn retry_wait_ms(retry_number: u32, retry_after_ms: Option<u64>, entropy: u64) -> u64 {
    if let Some(wait_ms) = retry_after_ms {
        return wait_ms;
    }
    let exponent = retry_number.saturating_sub(1);
    let multiplier = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
    jittered_wait_ms(BASE_WAIT_MS.saturating_mul(multiplier), entropy)
}

/// Apply ±20% jitter to a backoff wait so concurrent agents don't retry in lockstep.
/// `entropy` supplies the spread and is injected so the jitter band stays unit-testable.
fn jittered_wait_ms(base_ms: u64, entropy: u64) -> u64 {
    let span = base_ms / 5; // ±20%
    if span == 0 {
        return base_ms;
    }
    let offset = (entropy % (2 * span + 1)) as i64 - span as i64;
    (base_ms as i64 + offset).max(0) as u64
}

/// Well-spread entropy for retry jitter. The per-process seed decorrelates separate
/// agent processes; the counter decorrelates successive retries within one process.
fn retry_entropy() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    static SEED: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    let seed = *SEED.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
    });
    let mut z = seed ^ SEQ.fetch_add(1, Ordering::Relaxed);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod jitter_tests {
    use super::{jittered_wait_ms, retry_wait_ms};

    #[test]
    fn retry_after_is_not_exponentially_inflated() {
        assert_eq!(retry_wait_ms(1, Some(30_000), 0), 30_000);
        assert_eq!(retry_wait_ms(6, Some(30_000), u64::MAX), 30_000);
    }

    #[test]
    fn default_wait_uses_exponential_backoff_before_jitter() {
        assert_eq!(retry_wait_ms(1, None, 400), 2_000);
        assert_eq!(retry_wait_ms(3, None, 1_600), 8_000);
    }

    #[test]
    fn stays_within_20_percent_band() {
        for entropy in 0..10_000u64 {
            let w = jittered_wait_ms(2000, entropy);
            assert!(
                (1600..=2400).contains(&w),
                "wait {w} left ±20% band for entropy {entropy}"
            );
        }
    }

    #[test]
    fn spans_the_full_band() {
        let (mut lo, mut hi) = (u64::MAX, 0u64);
        for entropy in 0..10_000u64 {
            let w = jittered_wait_ms(2000, entropy);
            lo = lo.min(w);
            hi = hi.max(w);
        }
        assert_eq!(lo, 1600, "jitter should reach the lower bound");
        assert_eq!(hi, 2400, "jitter should reach the upper bound");
    }

    #[test]
    fn tiny_base_is_left_unchanged() {
        assert_eq!(jittered_wait_ms(4, 987_654), 4);
    }
}
