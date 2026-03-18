use async_trait::async_trait;

use crate::agent_input::AgentInput;
use crate::envelope::Envelope;
use crate::error::Result;
use crate::event::AgentEventPayload;
use crate::permission::PermissionDecision;

/// Unified abstraction for agent-to-consumer communication.
///
/// `UnifiedFrontend` is the sole implementation — it bridges the Data Plane
/// (Envelope mailbox) and Control Plane (ControlCommand channel) into
/// the `AgentInput`-based interface consumed by the agent loop.
///
/// ## Emission semantics
///
/// `emit()` behaviour depends on the agent role:
/// - **Root agent**: propagates errors (TUI disconnect is fatal).
/// - **Sub-agent**: best-effort — silently drops events if the parent
///   channel is closed, so that a dying parent does not crash children.
///
/// Callers should NOT rely on `emit()` failures for control flow.
#[async_trait]
pub trait AgentFrontend: Send + Sync {
    /// Emit a payload to the observer (TUI or parent agent).
    ///
    /// Best-effort for sub-agents: may silently succeed even if the
    /// event was not delivered. See trait-level documentation.
    async fn emit(&self, payload: AgentEventPayload) -> Result<()>;

    /// Wait for the next input. Returns `None` on disconnect,
    /// cancellation, or channel close (shutdown signal).
    async fn recv_input(&self) -> Option<AgentInput>;

    /// Atomic permission request — combines event emission and response
    /// waiting. Only called when `PermissionMode::check()` returns `Ask`.
    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision;

    /// Create a cloneable event emitter for use in `tokio::spawn` blocks.
    fn event_emitter(&self) -> Box<dyn EventEmitter>;

    /// Non-blocking drain of pending messages from the mailbox.
    ///
    /// Returns raw `Envelope`s preserving full source metadata.
    /// Called between tool executions and before sub-agent exit to
    /// prevent message loss. Default returns empty — root agent uses
    /// TUI Inbox instead.
    async fn drain_pending(&self) -> Vec<Envelope> {
        Vec::new()
    }
}

/// Lightweight, `Send + Sync` event emitter for parallel tool execution.
///
/// Best-effort: errors are logged but not propagated, since tool tasks
/// may outlive the TUI or parent agent.
#[async_trait]
pub trait EventEmitter: Send + Sync {
    /// Emit a payload (best-effort in spawned tasks).
    async fn emit(&self, payload: AgentEventPayload) -> Result<()>;
}
