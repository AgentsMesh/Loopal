use async_trait::async_trait;

use crate::agent_input::AgentInput;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_protocol::{Envelope, Question};
use loopal_tool_api::PermissionDecision;

/// Unified abstraction for agent-to-consumer communication.
///
/// Production uses `HubFrontend` (in `loopal-agent-server`), which broadcasts
/// events to IPC clients and routes permissions via the primary connection.
/// `UnifiedFrontend` (in this crate) is an in-process channel-based
/// implementation used by the test harness.
///
/// ## Emission semantics
///
/// `emit()` behaviour depends on the agent role:
/// - **Root agent**: propagates errors (consumer disconnect is fatal).
/// - **Sub-agent**: best-effort — silently drops events if the parent
///   channel is closed, so that a dying parent does not crash children.
///
/// Callers should NOT rely on `emit()` failures for control flow.
#[async_trait]
pub trait AgentFrontend: Send + Sync {
    /// Emit a payload to the observer (consumer or parent agent).
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
    /// consumer inbox instead.
    async fn drain_pending(&self) -> Vec<Envelope> {
        Vec::new()
    }

    /// Ask the user questions via the frontend (AskUser tool interception).
    /// Default returns "(not supported)" for sub-agents.
    async fn ask_user(&self, _questions: Vec<Question>) -> Vec<String> {
        vec!["(not supported)".into()]
    }

    /// Non-blocking, synchronous event emission for use in `Drop` guards.
    ///
    /// Returns `true` if the event was enqueued, `false` if the channel
    /// was full or closed. Safe to call from non-async contexts (e.g. panic
    /// unwinding). Default returns `false`.
    fn try_emit(&self, _payload: AgentEventPayload) -> bool {
        false
    }
}

/// Lightweight, `Send + Sync` event emitter for parallel tool execution.
///
/// Best-effort: errors are logged but not propagated, since tool tasks
/// may outlive the consumer or parent agent.
#[async_trait]
pub trait EventEmitter: Send + Sync {
    /// Emit a payload (best-effort in spawned tasks).
    async fn emit(&self, payload: AgentEventPayload) -> Result<()>;
}
