use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::MiddlewareContext;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Execute the middleware pipeline. Returns false if the loop should break.
    pub async fn execute_middleware(&mut self) -> Result<bool> {
        // Resolve provider for summarization (used by SmartCompact middleware)
        let summarization_provider = self.params.kernel.resolve_provider(&self.params.model).ok();

        let mut mw_ctx = MiddlewareContext {
            messages: self.params.messages.clone(),
            system_prompt: self.params.system_prompt.clone(),
            model: self.params.model.clone(),
            total_input_tokens: self.total_input_tokens,
            total_output_tokens: self.total_output_tokens,
            total_cost: 0.0, // no longer tracked, kept for middleware interface compatibility
            max_context_tokens: self.max_context_tokens,
            summarization_provider,
        };

        let before = mw_ctx.messages.len();

        if let Err(e) = self.params.context_pipeline.execute(&mut mw_ctx).await {
            self.emit(AgentEventPayload::Error {
                message: e.to_string(),
            })
            .await?;
            return Ok(false);
        }

        // Apply any middleware modifications (e.g., compacted messages)
        self.params.messages = mw_ctx.messages;

        let after = self.params.messages.len();
        if after < before {
            let note = format!("[context compacted: {} → {} messages]\n", before, after);
            self.emit(AgentEventPayload::Stream { text: note }).await?;
        }

        Ok(true)
    }
}
