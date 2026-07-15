use async_trait::async_trait;

use crate::agent_input::AgentInput;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_protocol::Question;
use loopal_tool_api::PermissionDecision;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanApproval {
    Approve,
    Reject,
    ApproveWithEdits(String),
}

/// Agent → consumer event/control surface.
///
/// `emit` is fallible — the root agent loop uses `?` to abort when the
/// consumer channel closes. Parallel/background tasks call `emit_best_effort`.
///
/// `emit_in_turn` is the capability-checked variant: it builds the envelope
/// via `AgentEvent::for_agent_in_turn`, which **panics** (in any build
/// profile) if called outside a `TurnContext::scope_turn` scope. Use this
/// for emit sites that MUST be inside a turn so mis-scoped sites surface
/// loudly instead of silently emitting `turn_id=0`.
#[async_trait]
pub trait AgentFrontend: Send + Sync {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()>;

    /// Capability-checked emit: panics if called outside `scope_turn`.
    /// Implementations should override to construct the envelope via
    /// `AgentEvent::for_agent_in_turn` (panic-checked at build time).
    /// The default impl pre-checks via `require_current()` then delegates
    /// to `emit` — functionally identical inside a scope, but loses the
    /// architectural intent of "in-turn envelope construction".
    async fn emit_in_turn(&self, payload: AgentEventPayload) -> Result<()> {
        let _ctx = loopal_protocol::event_id::TurnContext::require_current();
        self.emit(payload).await
    }

    async fn emit_best_effort(&self, payload: AgentEventPayload, ctx: &str) {
        if let Err(e) = self.emit(payload).await {
            tracing::error!(ctx = ctx, error = %e, "event emit failed");
        }
    }

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
        PlanApproval::Reject
    }

    fn try_emit(&self, _payload: AgentEventPayload) -> bool {
        false
    }
}

#[async_trait]
pub trait EventEmitter: Send + Sync {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()>;

    async fn emit_best_effort(&self, payload: AgentEventPayload, ctx: &str) {
        if let Err(e) = self.emit(payload).await {
            tracing::error!(ctx = ctx, error = %e, "event emit failed");
        }
    }
}
