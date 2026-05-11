use async_trait::async_trait;

use crate::agent_input::AgentInput;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_protocol::Question;
use loopal_tool_api::PermissionDecision;

/// Outcome of a plan approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanApproval {
    /// User approved the plan as-is.
    Approve,
    /// User rejected — agent should revise and retry.
    Reject,
    /// User edited the plan content before approving.
    ApproveWithEdits(String),
}

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

    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision;

    fn event_emitter(&self) -> Box<dyn EventEmitter>;

    async fn drain_pending(&self) -> Vec<AgentInput> {
        Vec::new()
    }

    async fn ask_user(&self, _questions: Vec<Question>) -> loopal_protocol::UserQuestionResponse {
        loopal_protocol::UserQuestionResponse::unsupported(
            "",
            "AskUser not supported in this context",
        )
    }

    async fn request_plan_approval(&self, _plan_content: &str, _plan_path: &str) -> PlanApproval {
        PlanApproval::Approve
    }

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
