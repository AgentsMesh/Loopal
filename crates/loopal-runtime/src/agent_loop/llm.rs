use super::cancel::TurnCancel;
use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;
use loopal_error::Result;
use loopal_provider_api::ContinuationIntent;
use opentelemetry::KeyValue;
use std::time::Instant;
use tracing::info;

impl AgentLoopRunner {
    /// Stream the LLM response. `intent` carries continuation context that the
    /// provider translates into protocol details (e.g. Anthropic appends a
    /// synthetic User tail for non-prefill models).
    pub async fn stream_llm_with(
        &mut self,
        intent: Option<ContinuationIntent>,
        cancel: &TurnCancel,
    ) -> Result<LlmStreamResult> {
        if cancel.is_cancelled() {
            return Ok(LlmStreamResult {
                stream_error: true,
                ..Default::default()
            });
        }

        let mut chat_params = self.prepare_chat_params(intent)?;
        if let Some(store) = self
            .params
            .resource_store
            .clone()
            .or_else(crate::hydrate::resource_store)
        {
            crate::hydrate::hydrate_turn_images(
                &mut chat_params.turns,
                store.as_ref(),
                &self.params.session.id,
                self.params.deps.kernel.settings().images.max_bytes,
            )
            .await?;
        }
        let model = self.params.config.model();
        let provider = self.params.deps.kernel.resolve_provider(&model)?;

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
            model = %model, turns = chat_params.turns.len(),
            tools = chat_params.tools.len(), max_tokens = chat_params.max_tokens,
            thinking = ?chat_params.thinking, "LLM request"
        );

        let mut result = self
            .retry_stream_response(&chat_params, &*provider, cancel)
            .await?;

        self.emit_thinking_complete(&result).await?;
        result.preserve_residual_thinking();
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
            KeyValue::new("gen_ai.request.model", model),
            KeyValue::new("gen_ai.system", provider.name().to_string()),
        ];
        crate::otel_metrics::llm_duration().record(llm_duration.as_secs_f64(), llm_attrs);
        Ok(result)
    }
}
