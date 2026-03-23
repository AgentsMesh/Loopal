use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use loopal_error::LoopalError;

use crate::backend::Backend;
use crate::memory_channel::MemoryChannel;
use crate::permission::PermissionLevel;

use crate::output_tail::OutputTail;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn permission(&self) -> PermissionLevel;

    /// Pre-execution validation. Returns `Some(reason)` to block, `None` to allow.
    /// Called before permission prompt. Default: always allow.
    fn precheck(&self, _input: &serde_json::Value) -> Option<String> {
        None
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> std::result::Result<ToolResult, LoopalError>;
}

/// Execution context passed to every `Tool::execute` invocation.
pub struct ToolContext {
    /// I/O backend for all filesystem, process, and network operations.
    /// Use `backend.cwd()` to get the current working directory.
    pub backend: Arc<dyn Backend>,
    /// Session ID.
    pub session_id: String,
    /// Opaque shared state passed to tools — tools downcast via `Any`.
    pub shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    /// Pending cwd switch set by tools (e.g. EnterWorktree/ExitWorktree).
    /// The runner checks this after each tool batch and recreates the backend.
    ///
    /// **Concurrency note**: tools in the same batch execute in parallel via
    /// `JoinSet`. If multiple tools write to this field, last-write-wins.
    /// Only one cwd-switching tool should appear per batch (enforced by LLM).
    pub pending_cwd_switch: Arc<std::sync::Mutex<Option<PathBuf>>>,
    /// Memory channel for sending observations to the Memory Observer sidebar.
    /// `None` when auto-memory is disabled.
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
    /// Shared output tail for streaming progress (set by tool_exec for Bash).
    /// Bash reads this to decide whether to use `exec_streaming` vs `exec`.
    pub output_tail: Option<Arc<OutputTail>>,
}

impl Clone for ToolContext {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            session_id: self.session_id.clone(),
            shared: self.shared.clone(),
            pending_cwd_switch: self.pending_cwd_switch.clone(),
            memory_channel: self.memory_channel.clone(),
            output_tail: self.output_tail.clone(),
        }
    }
}

impl fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolContext")
            .field("cwd", &self.backend.cwd())
            .field("session_id", &self.session_id)
            .field("shared", &self.shared.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Output content
    pub content: String,
    /// Whether the tool execution resulted in an error
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Prefix returned by `AttemptCompletionTool`; used by the runner and session
/// layer to detect completion results.
pub const COMPLETION_PREFIX: &str = "Completion acknowledged: ";

/// Tool definition for sending to LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
