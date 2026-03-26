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
                        self.emit(AgentEventPayload::RetryCleared).await?;
                    }
                    return Ok(Box::pin(futures::stream::empty()));
                }
            };
            match stream_result {
                Ok(s) => {
                    if retry_count > 0 {
                        self.emit(AgentEventPayload::RetryCleared).await?;
                    }
                    return Ok(s);
                }
                Err(e) if e.is_retryable() && retry_count < MAX_RETRIES => {
                    retry_count += 1;
                    let wait_ms =
                        e.retry_after_ms().unwrap_or(BASE_WAIT_MS) * (1 << (retry_count - 1));
                    warn!(
                        retry = retry_count, max_retries = MAX_RETRIES,
                        wait_ms, error = %e, "retrying"
                    );
                    self.emit(AgentEventPayload::RetryError {
                        message: format!(
                            "{}. Retrying in {:.1}s",
                            e,
                            wait_ms as f64 / 1000.0,
                        ),
                        attempt: retry_count,
                        max_attempts: MAX_RETRIES,
                    })
                    .await?;
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(wait_ms)) => {}
                        _ = cancel.cancelled() => {
                            info!("cancelled during retry wait");
                            self.emit(AgentEventPayload::RetryCleared).await?;
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
        if result.thinking_text.is_empty() && result.thinking_tokens == 0 {
            return Ok(());
        }
        let token_count = if result.thinking_text.is_empty() {
            result.thinking_tokens
        } else {
            result
                .thinking_tokens
                .max(result.thinking_text.len() as u32 / 4)
        };
        self.emit(AgentEventPayload::ThinkingComplete { token_count })
            .await
    }
}
