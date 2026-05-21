use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::command::AgentMode;

/// Control-plane commands that affect agent behaviour without carrying data.
///
/// Separated from data messages (`Envelope`) to enforce the Data/Control plane
/// boundary. Sent via a dedicated `control_tx` channel, never through the
/// `MessageRouter`.
///
/// Shutdown is signalled by dropping the `control_tx` sender — the receiver
/// in `UnifiedFrontend::recv_input()` returns `None`, terminating the loop.
#[derive(Debug, Clone, Serialize, Deserialize, EnumIter)]
pub enum ControlCommand {
    /// Switch agent operating mode (Act / Plan).
    ModeSwitch(AgentMode),
    /// Clear all conversation history.
    Clear,
    /// Compact old messages via LLM summarization. Optionally include
    /// user-provided focus hints (`/compact <instructions>`).
    Compact {
        #[serde(default)]
        instructions: Option<String>,
    },
    /// Switch to a different model at runtime.
    ModelSwitch(String),
    /// Rewind conversation to a specific turn (0-indexed from oldest).
    /// Discards the target turn and all subsequent messages.
    Rewind {
        turn_index: usize,
    },
    /// Switch thinking config at runtime. JSON string of ThinkingConfig.
    ThinkingSwitch(String),
    /// Resume (hot-swap) to a different persisted session by ID.
    ResumeSession(String),
    /// Request MCP server status snapshot (agent responds with McpStatusReport event).
    QueryMcpStatus,
    /// Reconnect a specific MCP server by name.
    McpReconnect {
        server: String,
    },
    /// Disconnect a specific MCP server by name.
    McpDisconnect {
        server: String,
    },
    /// Create a new thread goal. Fails if a goal already exists.
    GoalCreate {
        objective: String,
    },
    /// User-initiated lifecycle change. The runtime validates allowed
    /// transitions; illegal targets are rejected without changing state.
    GoalUserPause,
    GoalUserResume,
    GoalUserComplete,
    GoalUserReopen,
    GoalClear,
    /// Session-level suspend: gate closes (no deadline), `select_input`
    /// stops consuming cron/rewake; only human input resumes.
    Suspend,
    /// Counterpart to `Suspend`. Reopens the continuation gate and resumes
    /// cron/rewake consumption. Named `Unsuspend` (not `Resume`) to avoid
    /// collision with `ResumeSession` (session swap — a different concept).
    Unsuspend,
    /// Kill a running background shell task by its store ID.
    BgTaskKill {
        id: String,
    },
    /// Cancel a scheduled cron job by its scheduler-generated ID.
    CronDelete {
        id: String,
    },
}
