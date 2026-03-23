pub mod thinking;

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_tool_api::ToolDefinition;

pub use thinking::*;

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    async fn stream_chat(
        &self,
        params: &ChatParams,
    ) -> std::result::Result<ChatStream, LoopalError>;
}

pub type ChatStream = Pin<
    Box<dyn futures::Stream<Item = std::result::Result<StreamChunk, LoopalError>> + Send + Unpin>,
>;

#[derive(Debug, Clone)]
pub struct ChatParams {
    pub model: String,
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
    /// Directory for dumping failed API request bodies (diagnosis).
    /// Typically `locations::tmp_dir()`. `None` disables dumping.
    pub debug_dump_dir: Option<PathBuf>,
}

impl ChatParams {
    /// Convenience constructor with sensible defaults for optional fields.
    pub fn new(model: String, messages: Vec<Message>, system_prompt: String) -> Self {
        Self {
            model,
            messages,
            system_prompt,
            tools: vec![],
            max_tokens: 16_384,
            temperature: None,
            thinking: None,
            debug_dump_dir: None,
        }
    }
}

/// Why the LLM stopped generating output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// Model finished its response naturally.
    EndTurn,
    /// Output was truncated because it hit the max_tokens limit.
    MaxTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamChunk {
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    ThinkingSignature {
        signature: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        cache_creation_input_tokens: u32,
        cache_read_input_tokens: u32,
        thinking_tokens: u32,
    },
    Done {
        stop_reason: StopReason,
    },
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub input_price_per_mtok: f64,
    pub output_price_per_mtok: f64,
    pub thinking: ThinkingCapability,
}

// ---------------------------------------------------------------------------
// Middleware trait
// ---------------------------------------------------------------------------

/// Context passed through the middleware pipeline
pub struct MiddlewareContext {
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub model: String,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cost: f64,
    pub max_context_tokens: u32,
    /// Model for compaction/summarization. If None, uses `model`.
    pub compact_model: Option<String>,
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
    async fn process(&self, ctx: &mut MiddlewareContext) -> std::result::Result<(), LoopalError>;
}
