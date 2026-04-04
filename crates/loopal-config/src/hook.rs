use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Hook event types — lifecycle points where hooks can intercept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// When a session starts
    SessionStart,
    /// When a session ends
    SessionEnd,
    /// Right before the agent concludes its response (exit-gate)
    Stop,
    /// Before conversation compaction
    PreCompact,
}

/// Hook executor type — determines how the hook runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    /// Shell command (default, backward compatible).
    #[default]
    Command,
    /// HTTP POST to a webhook URL.
    Http,
    /// LLM prompt hook (lightweight classifier call).
    Prompt,
}

/// Hook configuration.
///
/// Backward compatible: `{"event": "pre_tool_use", "command": "echo hi"}`
/// still works (type defaults to Command, url/prompt/headers ignored).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub event: HookEvent,
    /// Executor type (default: command).
    #[serde(default, rename = "type")]
    pub hook_type: HookType,
    /// Shell command (Command type). Ignored for Http/Prompt types (leave empty).
    #[serde(default)]
    pub command: String,
    /// Webhook URL (required for Http type).
    #[serde(default)]
    pub url: Option<String>,
    /// HTTP headers (Http type only).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// LLM prompt (required for Prompt type).
    #[serde(default)]
    pub prompt: Option<String>,
    /// LLM model override (Prompt type only).
    #[serde(default)]
    pub model: Option<String>,
    /// Legacy tool_filter (use `condition` instead).
    #[serde(default)]
    pub tool_filter: Option<Vec<String>>,
    /// Condition expression: "Bash(git push*)", "Write(*.rs)", "*"
    #[serde(default, rename = "if")]
    pub condition: Option<String>,
    /// Timeout in milliseconds (default: 10000).
    #[serde(default = "default_hook_timeout")]
    pub timeout_ms: u64,
    /// Deduplication ID across config layers.
    #[serde(default)]
    pub id: Option<String>,
}

fn default_hook_timeout() -> u64 {
    10_000
}

/// Result from hook execution (legacy, used by runner.rs).
#[derive(Debug, Clone)]
pub struct HookResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl HookResult {
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}
