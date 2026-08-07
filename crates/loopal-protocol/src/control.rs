use std::time::Duration;

use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::command::AgentMode;

/// Window in which agent/control reports synchronous application. After this
/// deadline the server responds `queued` but retains the execution lease.
pub const DEFAULT_CONTROL_APPLICATION_TIMEOUT: Duration = Duration::from_secs(3);

/// Scheduling/transport headroom after the synchronous application window.
pub const CONTROL_RPC_COMPLETION_GRACE: Duration = Duration::from_secs(2);

/// End-to-end Hub-to-agent RPC bound. This must remain strictly larger than
/// `DEFAULT_CONTROL_APPLICATION_TIMEOUT`, otherwise the Hub can report an
/// indeterminate outcome before the server reports accepted queued work.
pub const DEFAULT_CONTROL_RPC_TIMEOUT: Duration = Duration::from_secs(
    DEFAULT_CONTROL_APPLICATION_TIMEOUT.as_secs() + CONTROL_RPC_COMPLETION_GRACE.as_secs(),
);
const _: () = assert!(
    DEFAULT_CONTROL_RPC_TIMEOUT.as_millis() > DEFAULT_CONTROL_APPLICATION_TIMEOUT.as_millis()
);

/// Authoritative outcome of a control request across agent, Hub, and UI RPC.
///
/// `Queued` means the runtime owns the request but has not reached the turn
/// boundary where it can apply it. `Unknown` means the forwarding deadline
/// elapsed after sending, so callers must not infer either success or failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ControlDisposition {
    Applied,
    Queued,
    Unknown,
    Rejected { reason: String },
}

impl ControlDisposition {
    /// Parse the typed wire contract while accepting pre-disposition agents.
    /// Legacy `{ "ok": true }` only proved queue acceptance, not application.
    pub fn from_wire_value(value: serde_json::Value) -> Result<Self, String> {
        if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
            return Ok(Self::Queued);
        }
        if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
            let reason = value
                .get("reason")
                .or_else(|| value.get("error"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("legacy agent rejected control")
                .to_string();
            return Ok(Self::Rejected { reason });
        }
        serde_json::from_value(value)
            .map_err(|error| format!("invalid control disposition: {error}"))
    }
}

/// Control-plane commands that affect agent behaviour without carrying data.
///
/// Separated from data messages (`Envelope`) to enforce the Data/Control plane
/// boundary. Sent via a dedicated `control_tx` channel, never through the
/// `MessageRouter`.
///
/// Shutdown is signalled by a cancellation token or by closing both data and
/// control input channels. A single closed plane does not terminate the other.
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
    /// Switch permission mode at runtime (bypass / ask_dangerous / ask_any_write).
    PermissionModeSwitch(String),
    /// Switch decision mode at runtime (manual / classifier / agent).
    DecisionModeSwitch(String),
    /// Switch sandbox policy at runtime (disabled / default_write / read_only).
    SandboxPolicySwitch(String),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarding_deadline_outlives_application_window() {
        assert!(DEFAULT_CONTROL_RPC_TIMEOUT > DEFAULT_CONTROL_APPLICATION_TIMEOUT);
        assert!(
            DEFAULT_CONTROL_RPC_TIMEOUT - DEFAULT_CONTROL_APPLICATION_TIMEOUT
                >= CONTROL_RPC_COMPLETION_GRACE
        );
    }

    #[test]
    fn disposition_wire_contract_is_typed_and_legacy_compatible() {
        for disposition in [
            ControlDisposition::Applied,
            ControlDisposition::Queued,
            ControlDisposition::Unknown,
            ControlDisposition::Rejected {
                reason: "unsupported".into(),
            },
        ] {
            let value = serde_json::to_value(&disposition).unwrap();
            assert_eq!(
                ControlDisposition::from_wire_value(value).unwrap(),
                disposition
            );
        }
        assert_eq!(
            ControlDisposition::from_wire_value(serde_json::json!({"ok": true})).unwrap(),
            ControlDisposition::Queued
        );
        assert_eq!(
            ControlDisposition::from_wire_value(
                serde_json::json!({"ok": false, "error": "old rejection"})
            )
            .unwrap(),
            ControlDisposition::Rejected {
                reason: "old rejection".into()
            }
        );
        assert!(ControlDisposition::from_wire_value(serde_json::json!({"ok": "yes"})).is_err());
    }
}
