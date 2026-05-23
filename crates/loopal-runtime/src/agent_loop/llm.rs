use super::cancel::TurnCancel;
use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;
use futures::StreamExt;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContinuationIntent;
use loopal_provider_api::Message;
use opentelemetry::KeyValue;
use std::time::Instant;
use tracing::{info, warn};

impl AgentLoopRunner {
    /// Stream the LLM response. `intent` carries continuation context that the
    /// provider translates into protocol details (e.g. Anthropic appends a
    /// synthetic User tail for non-prefill models).
    pub async fn stream_llm_with(
        &mut self,
        messages: &[Message],
        intent: Option<ContinuationIntent>,
        cancel: &TurnCancel,
    ) -> Result<LlmStreamResult> {
        if cancel.is_cancelled() {
            return Ok(LlmStreamResult {
                stream_error: true,
                ..Default::default()
            });
        }

        let mut chat_params = self.prepare_chat_params_with(messages, intent)?;
        if let Some(store) = crate::hydrate::resource_store() {
            crate::hydrate::hydrate_images(
                &mut chat_params.messages,
                store.as_ref(),
                &self.params.session.id,
            )
            .await?;
        }
        let provider = self
            .params
            .deps
            .kernel
            .resolve_provider(self.params.config.model())?;

        // PreRequest hook: notify before LLM call.
        crate::fire_hooks::fire_hooks(
            &self.params.deps.kernel,
            loopal_config::HookEvent::PreRequest,
            &loopal_hooks::HookContext {
                session_id: Some(&self.params.session.id),
                ..Default::default()
            },
        )
        .await;

        let llm_start = Instant::now();
        info!(
            model = %self.params.config.model(), messages = messages.len(),
            tools = chat_params.tools.len(), max_tokens = chat_params.max_tokens,
            thinking = ?chat_params.thinking, "LLM request"
        );

        let mut stream = self
            .retry_stream_chat(&chat_params, &*provider, cancel)
            .await?;
        let mut result = LlmStreamResult::default();
        let mut received_done = false;

        loop {
            tokio::select! {
                biased;
                chunk = stream.next() => {
                    let Some(chunk_result) = chunk else { break; };
                    if !self.handle_stream_chunk(chunk_result, &mut result, &mut received_done).await? {
                        break;
                    }
                }
                _ = cancel.cancelled() => {
                    info!("cancelled during LLM streaming");
                    result.stream_error = true;
                    break;
                }
            }
        }

        // Stream EOF without Done → connection dropped mid-stream.
        // Exclude cancellation: retry_stream_chat returns empty stream on cancel,
        // which would look like truncation but is intentional.
        if !received_done && !result.stream_error && !cancel.is_cancelled() {
            warn!("SSE stream ended without message_stop — treating as stream truncation");
            self.emit_in_turn(AgentEventPayload::Error {
                message: "Response stream ended unexpectedly — possible network interruption"
                    .to_string(),
            })
            .await?;
            result.stream_error = true;
        }

        self.emit_thinking_complete(&result).await?;
        let llm_duration = llm_start.elapsed();
        info!(
            duration_ms = llm_duration.as_millis() as u64,
            tool_calls = result.tool_uses.len(),
            server_blocks = result.server_blocks.len(),
            has_text = !result.assistant_text.is_empty(),
            thinking_tokens = result.thinking_tokens,
            "LLM complete"
        );
        let llm_attrs = &[
            KeyValue::new(
                "gen_ai.request.model",
                self.params.config.model().to_string(),
            ),
            KeyValue::new("gen_ai.system", provider.name().to_string()),
        ];
        crate::otel_metrics::llm_duration().record(llm_duration.as_secs_f64(), llm_attrs);
        Ok(result)
    }
}
