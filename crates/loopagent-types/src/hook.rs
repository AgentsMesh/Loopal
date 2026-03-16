use serde::{Deserialize, Serialize};

/// Hook event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Before tool execution
    PreToolUse,
    /// After tool execution
    PostToolUse,
    /// Before sending to LLM
    PreRequest,
    /// After user submits input
    PostInput,
}

/// Hook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// Event that triggers this hook
    pub event: HookEvent,
    /// Shell command to execute
    pub command: String,
    /// Optional: only trigger for specific tool names
    #[serde(default)]
    pub tool_filter: Option<Vec<String>>,
    /// Timeout in milliseconds (default: 10000)
    #[serde(default = "default_hook_timeout")]
    pub timeout_ms: u64,
}

fn default_hook_timeout() -> u64 {
    10_000
}

/// Result from hook execution
#[derive(Debug, Clone)]
pub struct HookResult {
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Stdout output
    pub stdout: String,
    /// Stderr output
    pub stderr: String,
}

impl HookResult {
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}
