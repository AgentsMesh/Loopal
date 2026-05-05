pub mod model;
pub mod model_router;
pub mod thinking;

use std::borrow::Cow;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_tool_api::ToolDefinition;

pub use model::*;
pub use model_router::ModelRouter;
pub use thinking::*;

// ---------------------------------------------------------------------------
// Continuation intent (Runtime → Provider)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuationReason {
    MaxTokensWithoutTools,
    MaxTokensWithTools,
    PauseTurn,
    StreamTruncated,
    /// Re-entering ReadyToCall after `try_recover` modified the store
    /// (compaction / server-block condense). The previous turn's intent was
    /// lost when its `TurnContext` ended; this variant re-establishes the
    /// `last==User || intent.is_some()` invariant.
    RecoveryRetry,
}

/// Runtime-expressed continuation intent passed to a Provider via `ChatParams`.
/// `reason` is informational metadata (logging / telemetry); the protocol-level
/// effect of `intent.is_some()` is the same regardless of variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuationIntent {
    AutoContinue { reason: ContinuationReason },
}

// ---------------------------------------------------------------------------
// Error classification (Provider → Runtime)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    Retryable,
    ContextOverflow,
    ServerBlockError,
    PrefillRejected,
    Fatal,
}

/// Generic error → ErrorClass mapping based only on `ProviderError` variant.
/// Used as the trait default and as the fallback tail of every provider's
/// override (after their protocol-specific keyword matching).
pub fn default_classify_error(err: &LoopalError) -> ErrorClass {
    if err.is_context_overflow() {
        ErrorClass::ContextOverflow
    } else if err.is_retryable() {
        ErrorClass::Retryable
    } else {
        ErrorClass::Fatal
    }
}

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

    /// Protocol-level message finalization. Called by `stream_chat` to enforce
    /// per-provider invariants (e.g. Anthropic requires User tail when
    /// `supports_prefill==false` or `continuation_intent.is_some()`). Default: passthrough.
    fn finalize_messages<'a>(&self, params: &'a ChatParams) -> Cow<'a, [Message]> {
        Cow::Borrowed(&params.messages)
    }

    /// Map a provider error to a recovery class. Default uses
    /// `default_classify_error` (variant-only, no protocol strings).
    /// Providers should override to add protocol-specific keyword matching,
    /// then delegate to `default_classify_error` as the fallback tail.
    fn classify_error(&self, err: &LoopalError) -> ErrorClass {
        default_classify_error(err)
    }
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
    /// Runtime-expressed continuation intent. Provider translates into protocol.
    /// Never persisted; never enters ContextStore.
    pub continuation_intent: Option<ContinuationIntent>,
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
            continuation_intent: None,
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
    async fn process(&self, ctx: &mut MiddlewareContext) -> std::result::Result<(), LoopalError>;
}
