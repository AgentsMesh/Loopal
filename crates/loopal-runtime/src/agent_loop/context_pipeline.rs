use loopal_message::Message;
use loopal_provider_api::MiddlewareContext;
use tracing::warn;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Run the context middleware pipeline on a working copy of messages.
    /// Non-fatal: pipeline errors are logged and swallowed so the LLM call proceeds.
    pub async fn run_context_pipeline(&self, messages: &mut Vec<Message>) {
        let mut ctx = MiddlewareContext {
            messages: std::mem::take(messages),
            system_prompt: self.params.config.system_prompt.clone(),
            model: self.params.config.model().to_string(),
            total_input_tokens: self.tokens.input,
            total_output_tokens: self.tokens.output,
            max_context_tokens: self.params.store.budget().context_window,
            summarization_provider: None,
        };
        if let Err(e) = self.pipeline.execute(&mut ctx).await {
            warn!(error = %e, "context pipeline failed, proceeding without refresh");
        }
        *messages = ctx.messages;
    }
}
