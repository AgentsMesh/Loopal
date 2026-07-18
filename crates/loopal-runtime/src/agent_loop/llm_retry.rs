use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ChatParams;
use tracing::{info, warn};

use super::cancel::TurnCancel;
use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Retry loop for the initial stream_chat API call.
    ///
    /// Exposed for integration testing. Production callers use `stream_llm_with`.
    pub async fn retry_stream_chat(
        &mut self,
        params: &ChatParams,
        provider: &dyn loopal_provider_api::Provider,
        cancel: &TurnCancel,
    ) -> Result<loopal_provider_api::ChatStream> {
        const MAX_RETRIES: u32 = 6;
        const BASE_WAIT_MS: u64 = 2000;
        let mut retry_count = 0;
        loop {
            if cancel.is_cancelled() {
                return Ok(Box::pin(futures::stream::empty()));
            }
            let stream_result = tokio::select! {
                biased;
                result = provider.stream_chat(params) => result,
                _ = cancel.cancelled() => {
                    if retry_count > 0 {
                        self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                    }
                    return Ok(Box::pin(futures::stream::empty()));
                }
            };
            match stream_result {
                Ok(s) => {
                    if retry_count > 0 {
                        self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                    }
                    return Ok(s);
                }
                Err(e) if e.is_retryable() && retry_count < MAX_RETRIES => {
                    retry_count += 1;
                    let exp = 1u64 << (retry_count - 1);
                    // reason: honor an explicit Retry-After as the server directs; add ±20%
                    // jitter only to the default exponential backoff so many agents retrying
                    // after a shared outage don't reconverge into a synchronized storm.
                    let wait_ms = match e.retry_after_ms() {
                        Some(after_ms) => after_ms * exp,
                        None => jittered_wait_ms(BASE_WAIT_MS * exp, retry_entropy()),
                    };
                    warn!(
                        retry = retry_count, max_retries = MAX_RETRIES,
                        wait_ms, error = %e, "retrying"
                    );
                    self.emit_in_turn(AgentEventPayload::RetryError {
                        message: format!("{}. Retrying in {:.1}s", e, wait_ms as f64 / 1000.0,),
                        attempt: retry_count,
                        max_attempts: MAX_RETRIES,
                    })
                    .await?;
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(wait_ms)) => {}
                        _ = cancel.cancelled() => {
                            info!("cancelled during retry wait");
                            self.emit_in_turn(AgentEventPayload::RetryCleared).await?;
                            return Ok(Box::pin(futures::stream::empty()));
                        }
                    }
                }
                Err(e) => return Err(e),
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
    use super::jittered_wait_ms;

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
