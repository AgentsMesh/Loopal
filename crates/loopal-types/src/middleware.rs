use std::sync::Arc;

use async_trait::async_trait;

use crate::error::LoopalError;
use crate::message::Message;
use crate::provider::Provider;

/// Context passed through the middleware pipeline
pub struct MiddlewareContext {
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub model: String,
    pub turn_count: u32,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cost: f64,
    pub max_context_tokens: u32,
    /// Optional provider for LLM-based summarization during compaction.
    /// If None, fallback to traditional truncation.
    pub summarization_provider: Option<Arc<dyn Provider>>,
}

/// Middleware trait for the context pipeline
#[async_trait]
pub trait Middleware: Send + Sync {
    fn name(&self) -> &str;

    /// Process and potentially modify the middleware context.
    /// Return Err to abort the pipeline.
    async fn process(
        &self,
        ctx: &mut MiddlewareContext,
    ) -> std::result::Result<(), LoopalError>;
}
