use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::pin::Pin;

use crate::error::LoopAgentError;
use crate::message::Message;
use crate::tool::ToolDefinition;

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    async fn stream_chat(
        &self,
        params: &ChatParams,
    ) -> std::result::Result<ChatStream, LoopAgentError>;
}

pub type ChatStream = Pin<
    Box<
        dyn futures::Stream<Item = std::result::Result<StreamChunk, LoopAgentError>>
            + Send
            + Unpin,
    >,
>;

#[derive(Debug, Clone)]
pub struct ChatParams {
    pub model: String,
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    /// Directory for dumping failed API request bodies (diagnosis).
    /// Typically `locations::tmp_dir()`. `None` disables dumping.
    pub debug_dump_dir: Option<PathBuf>,
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
}
