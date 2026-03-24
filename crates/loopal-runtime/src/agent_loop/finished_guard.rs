//! Drop guard that ensures `Finished` event is always emitted.
//!
//! Placed at the `agent_loop()` entry point. On normal exit the caller
//! calls `disarm()` and the guard is a no-op. On panic (stack unwinding),
//! `Drop` fires and sends `Error` + `Finished` via the sync `try_emit`
//! path — no async runtime required.

use std::sync::Arc;

use loopal_protocol::AgentEventPayload;

use crate::frontend::traits::AgentFrontend;

/// Panic-safe guard for agent loop completion events.
///
/// Create at the top of `agent_loop()`, disarm before returning.
/// If the agent panics, `Drop` emits `Error` + `Finished` synchronously.
pub(super) struct FinishedGuard {
    frontend: Option<Arc<dyn AgentFrontend>>,
}

impl FinishedGuard {
    pub(super) fn new(frontend: Arc<dyn AgentFrontend>) -> Self {
        Self {
            frontend: Some(frontend),
        }
    }

    /// Disarm the guard after the normal exit path has already emitted `Finished`.
    pub(super) fn disarm(&mut self) {
        self.frontend = None;
    }
}

impl Drop for FinishedGuard {
    fn drop(&mut self) {
        if let Some(frontend) = self.frontend.take() {
            tracing::error!("agent loop terminated unexpectedly (panic); emitting Finished");
            frontend.try_emit(AgentEventPayload::Error {
                message: "Agent loop terminated unexpectedly".into(),
            });
            frontend.try_emit(AgentEventPayload::Finished);
        }
    }
}
