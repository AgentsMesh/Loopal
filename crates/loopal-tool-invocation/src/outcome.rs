use serde::{Deserialize, Serialize};

/// Terminal outcome of a tool invocation.
///
/// `Outcome` carries only the textual result and (for failures) the
/// failure kind. Structured side data — `ToolResultMetadata`'s
/// `Stale`/`Cancelled`/`BytesWritten`/`ModifiedFiles` variants — lives on the wrapping
/// `ToolInvocation.metadata`, NOT here. Single-sourced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outcome {
    Success { content: String },
    Failure { error: String, kind: FailureKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    ToolError,
    PermissionDenied,
    Interrupted,
    Watchdog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleReason {
    WatchdogTimeout,
    TurnEnded,
    ConnectionLost,
    /// The provider exposed a complete-looking tool call, but the surrounding
    /// model response never reached a valid terminal boundary. The runtime
    /// discarded the provisional call without executing it.
    IncompleteModelResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelCause {
    UserInterrupt,
    ParentCancelled,
    GovernanceAbort,
}

/// Typed metadata attached to `ToolResult`.
///
/// Replaces ad-hoc `serde_json::Value` with explicit variants so consumers
/// (view-state, ACP, session_display) can `match` exhaustively instead of
/// parsing JSON keys.
///
/// Wire format (adjacently tagged via `kind`):
/// - `{"kind": "stale", "reason": "watchdog_timeout"}`
/// - `{"kind": "cancelled", "cause": "user_interrupt"}`
/// - `{"kind": "bytes_written", "count": 1024}`
/// - `{"kind": "modified_files", "paths": ["/workspace/a.rs"]}`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolResultMetadata {
    Stale { reason: StaleReason },
    Cancelled { cause: CancelCause },
    BytesWritten { count: u64 },
    ModifiedFiles { paths: Vec<String> },
}

impl ToolResultMetadata {
    pub fn stale(reason: StaleReason) -> Self {
        Self::Stale { reason }
    }

    pub fn cancelled(cause: CancelCause) -> Self {
        Self::Cancelled { cause }
    }

    pub fn bytes_written(count: u64) -> Self {
        Self::BytesWritten { count }
    }

    pub fn modified_files(paths: Vec<String>) -> Self {
        Self::ModifiedFiles { paths }
    }
}

impl Outcome {
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Failure { .. })
    }

    pub fn content(&self) -> &str {
        match self {
            Self::Success { content, .. } => content,
            Self::Failure { error, .. } => error,
        }
    }
}

impl std::fmt::Display for StaleReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::WatchdogTimeout => "watchdog timeout",
            Self::TurnEnded => "turn ended",
            Self::ConnectionLost => "connection lost",
            Self::IncompleteModelResponse => "incomplete model response",
        };
        f.write_str(s)
    }
}

impl std::fmt::Display for CancelCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::UserInterrupt => "user interrupt",
            Self::ParentCancelled => "parent cancelled",
            Self::GovernanceAbort => "governance abort",
        };
        f.write_str(s)
    }
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ToolError => "tool error",
            Self::PermissionDenied => "permission denied",
            Self::Interrupted => "interrupted",
            Self::Watchdog => "watchdog",
        };
        f.write_str(s)
    }
}
