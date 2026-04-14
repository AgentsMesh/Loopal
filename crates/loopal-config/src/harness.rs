use serde::{Deserialize, Serialize};

/// Control-loop parameters for the agent harness.
///
/// All fields have sensible defaults matching the previous hardcoded values.
/// Override via `settings.json` under the `"harness"` key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HarnessConfig {
    /// Loop detection: warn after this many repeated tool calls (default: 3).
    pub loop_warn_threshold: u32,
    /// Loop detection: abort turn after this many repeats (default: 5).
    pub loop_abort_threshold: u32,
    /// Auto-mode circuit breaker: max consecutive denials per tool (default: 3).
    pub cb_max_consecutive_denials: u32,
    /// Auto-mode circuit breaker: max total denials per session (default: 20).
    pub cb_max_total_denials: u32,
    /// Max automatic continuations when LLM hits max_tokens (default: 3).
    pub max_auto_continuations: u32,
    /// Max Stop hook feedback rounds before forcing exit (default: 2).
    pub max_stop_feedback: u32,
    /// Max agent spawn depth (default: 2). Agents at depth >= this value
    /// lose their spawn tools. E.g., 2 means depth 0 and 1 can spawn,
    /// depth 2+ cannot.
    pub agent_max_depth: u32,
    /// Max total sub-agents across the entire Hub (default: 16).
    /// Excludes root agent — only counts spawned children.
    pub agent_max_total: u32,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            loop_warn_threshold: 3,
            loop_abort_threshold: 5,
            cb_max_consecutive_denials: 3,
            cb_max_total_denials: 20,
            max_auto_continuations: 3,
            max_stop_feedback: 2,
            agent_max_depth: 2,
            agent_max_total: 16,
        }
    }
}
