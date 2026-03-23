use loopal_error::Result;
use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::MiddlewareContext;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Execute the middleware pipeline on the persistent history.
    /// Works on a clone — persistent history is never modified.
    /// Returns false if the loop should break (middleware error).
    pub async fn execute_middleware(&mut self) -> Result<bool> {
        let mut working = self.params.messages.clone();
        self.execute_middleware_on(&mut working).await
    }

    /// Execute the middleware pipeline on a provided working copy.
    /// The caller owns `working` and decides what to do with the result.
    pub async fn execute_middleware_on(&mut self, working: &mut Vec<Message>) -> Result<bool> {
        // Resolve summarization provider from compact_model (fallback to main model)
        let compact_model = self
            .params
            .compact_model
            .as_deref()
            .unwrap_or(&self.params.model);
        let summarization_provider = self.params.kernel.resolve_provider(compact_model).ok();

        let mut mw_ctx = MiddlewareContext {
            messages: working.clone(),
            system_prompt: self.params.system_prompt.clone(),
            model: self.params.model.clone(),
            total_input_tokens: self.tokens.input,
            total_output_tokens: self.tokens.output,
            total_cost: 0.0,
            max_context_tokens: self.model_config.max_context_tokens,
            compact_model: self.params.compact_model.clone(),
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

        *working = mw_ctx.messages;

        let after = working.len();
        if after < before {
            let note = format!("[context compacted: {before} → {after} messages]\n");
            self.emit(AgentEventPayload::Stream { text: note }).await?;
        }

        Ok(true)
    }
}
