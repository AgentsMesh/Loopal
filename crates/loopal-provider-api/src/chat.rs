use std::path::PathBuf;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_tool_api::ToolDefinition;
use loopal_turn::Turn;

use crate::ContinuationIntent;
use crate::thinking::ThinkingConfig;

pub type ChatStream = Pin<
    Box<dyn futures::Stream<Item = std::result::Result<StreamChunk, LoopalError>> + Send + Unpin>,
>;

#[derive(Debug, Clone)]
pub struct ChatParams {
    pub model: String,
    pub messages: Vec<Message>,
    /// Domain-shaped conversation history (new SSOT). When non-empty,
    /// providers should fold this directly into wire-format JSON; the
    /// `messages` field is kept as fallback during the dual-write
    /// transition (PR-5a) and will be removed in PR-6.
    pub turns: Vec<Turn>,
    pub system_prompt: String,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
    /// Runtime-expressed continuation intent. Provider translates into protocol.
    /// Never persisted; never enters ContextStore.
    pub continuation_intent: Option<ContinuationIntent>,
    /// Directory for dumping failed API request bodies (diagnosis).
    /// Typically `locations::tmp_dir()`. `None` disables dumping.
    pub debug_dump_dir: Option<PathBuf>,
}

impl ChatParams {
    pub fn new(model: String, messages: Vec<Message>, system_prompt: String) -> Self {
        Self {
            model,
            messages,
            turns: Vec::new(),
            system_prompt,
            tools: vec![],
            max_tokens: 16_384,
            temperature: None,
            thinking: None,
            continuation_intent: None,
            debug_dump_dir: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    /// Server-side tool hit iteration limit; client should send assistant
    /// message back to continue. Currently Anthropic-only (`pause_turn`).
    PauseTurn,
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
    /// Server-side tool invocation (e.g. web_search). NOT executed by client.
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Server-side tool result. `block_type` is the original API type string,
    /// e.g. "web_search_tool_result", "code_execution_tool_result".
    ServerToolResult {
        block_type: String,
        tool_use_id: String,
        content: serde_json::Value,
    },
}
