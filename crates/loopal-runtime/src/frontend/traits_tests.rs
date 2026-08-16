use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_error::{LoopalError, Result};
use loopal_protocol::{AgentEventPayload, PermissionIntentRequest, UserQuestionResponse};
use loopal_tool_api::PermissionDecision;

use super::{AgentFrontend, EventEmitter, PlanApproval, PlanApprovalCancellationReason};
use crate::agent_input::AgentInput;

struct DefaultsFrontend {
    fail: AtomicBool,
    emitted: AtomicUsize,
}

impl DefaultsFrontend {
    fn new(fail: bool) -> Self {
        Self {
            fail: AtomicBool::new(fail),
            emitted: AtomicUsize::new(0),
        }
    }
}

struct DefaultsEmitter {
    fail: bool,
    emitted: Arc<AtomicUsize>,
}

#[async_trait]
impl EventEmitter for DefaultsEmitter {
    async fn emit(&self, _payload: AgentEventPayload) -> Result<()> {
        if self.fail {
            Err(LoopalError::Other("emitter closed".into()))
        } else {
            self.emitted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
}

#[async_trait]
impl AgentFrontend for DefaultsFrontend {
    async fn emit(&self, _payload: AgentEventPayload) -> Result<()> {
        if self.fail.load(Ordering::Acquire) {
            Err(LoopalError::Other("frontend closed".into()))
        } else {
            self.emitted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        None
    }

    async fn try_recv_input(
        &self,
    ) -> std::result::Result<AgentInput, tokio::sync::mpsc::error::TryRecvError> {
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
    }

    async fn request_permission(&self, _request: &PermissionIntentRequest) -> PermissionDecision {
        PermissionDecision::Deny
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(DefaultsEmitter {
            fail: false,
            emitted: Arc::new(AtomicUsize::new(0)),
        })
    }
}

fn permission_request() -> PermissionIntentRequest {
    PermissionIntentRequest::create(
        "call-defaults",
        "Read",
        serde_json::json!({"path": "README.md"}),
        serde_json::json!({"path": "README.md"}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap()
}

#[tokio::test]
async fn frontend_defaults_cover_permission_input_and_plan_fallbacks() {
    let frontend = DefaultsFrontend::new(false);
    let outcome = frontend
        .request_permission_outcome(&permission_request())
        .await;
    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(outcome.reason.is_empty());
    assert_eq!(outcome.duration_ms, 0);
    assert!(outcome.receipt.is_none());
    assert!(frontend.drain_pending().await.is_empty());
    assert!(matches!(
        frontend.ask_user(Vec::new()).await,
        UserQuestionResponse::Unsupported { reason, .. }
            if reason == "AskUser not supported in this context"
    ));
    assert_eq!(
        frontend.request_plan_approval("plan", "plan.md").await,
        PlanApproval::Cancelled(PlanApprovalCancellationReason::Unavailable)
    );
    assert!(!frontend.try_emit(AgentEventPayload::Started));
}

#[tokio::test]
async fn frontend_default_emit_helpers_enforce_turn_scope_and_swallow_failures() {
    let frontend = DefaultsFrontend::new(false);
    loopal_protocol::event_id::scope_turn(42, frontend.emit_in_turn(AgentEventPayload::Started))
        .await
        .unwrap();
    frontend
        .emit_best_effort(AgentEventPayload::Finished, "frontend-success")
        .await;
    assert_eq!(frontend.emitted.load(Ordering::SeqCst), 2);

    frontend.fail.store(true, Ordering::Release);
    frontend
        .emit_best_effort(AgentEventPayload::Finished, "frontend-failure")
        .await;
    assert_eq!(frontend.emitted.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn event_emitter_default_best_effort_covers_success_and_failure() {
    let emitted = Arc::new(AtomicUsize::new(0));
    let success = DefaultsEmitter {
        fail: false,
        emitted: emitted.clone(),
    };
    success
        .emit_best_effort(AgentEventPayload::Started, "emitter-success")
        .await;
    let failure = DefaultsEmitter {
        fail: true,
        emitted: emitted.clone(),
    };
    failure
        .emit_best_effort(AgentEventPayload::Finished, "emitter-failure")
        .await;
    assert_eq!(emitted.load(Ordering::SeqCst), 1);
}
