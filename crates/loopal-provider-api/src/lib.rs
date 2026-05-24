pub mod chat;
pub mod middleware;
pub mod model;
pub mod model_router;
pub mod resolver;
pub mod thinking;
pub mod wire;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use loopal_error::LoopalError;

pub use chat::{ChatParams, ChatStream, StopReason, StreamChunk};
pub use middleware::{Middleware, MiddlewareContext};
pub use model::*;
pub use model_router::ModelRouter;
pub use resolver::ProviderResolver;
pub use thinking::*;
pub use wire::{
    ContentBlock, ImageSource, Message, MessageOrigin, MessageRole, normalize_messages,
    project_turn_to_messages, project_turns_to_messages,
};

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

    /// Map a provider error to a recovery class. Default uses
    /// `default_classify_error` (variant-only, no protocol strings).
    /// Providers should override to add protocol-specific keyword matching,
    /// then delegate to `default_classify_error` as the fallback tail.
    fn classify_error(&self, err: &LoopalError) -> ErrorClass {
        default_classify_error(err)
    }
}
