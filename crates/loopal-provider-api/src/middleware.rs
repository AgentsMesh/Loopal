use std::sync::Arc;

use async_trait::async_trait;

use loopal_error::LoopalError;

use crate::Provider;
use crate::wire::Message;

pub struct MiddlewareContext {
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub model: String,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub max_context_tokens: u32,
    /// Optional provider for LLM-based summarization during compaction.
    /// If None, fallback to traditional truncation.
    pub summarization_provider: Option<Arc<dyn Provider>>,
}

#[async_trait]
pub trait Middleware: Send + Sync {
    fn name(&self) -> &str;

    /// Process and potentially modify the middleware context.
    /// Return Err to abort the pipeline.
    async fn process(&self, ctx: &mut MiddlewareContext) -> std::result::Result<(), LoopalError>;
}
