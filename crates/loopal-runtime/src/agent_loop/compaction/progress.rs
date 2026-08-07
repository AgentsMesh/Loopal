use std::sync::Arc;

use loopal_protocol::{AgentEventPayload, CompactPhase};

use crate::frontend::traits::AgentFrontend;

/// Owns one compaction progress lifecycle.
///
/// Normal paths call [`Self::finish`] so `Done` is delivered asynchronously.
/// If the future is cancelled or unwinds after the start event, `Drop` makes a
/// synchronous best-effort attempt to close the lifecycle instead of leaving a
/// persistent compacting banner behind.
#[must_use = "the progress guard must be retained until the compaction lifecycle finishes"]
pub(super) struct CompactProgressGuard {
    frontend: Arc<dyn AgentFrontend>,
    armed: bool,
}

impl CompactProgressGuard {
    pub(super) async fn start(
        frontend: Arc<dyn AgentFrontend>,
        phase: CompactPhase,
        detail: Option<String>,
    ) -> Self {
        debug_assert_ne!(phase, CompactPhase::Done);
        let guard = Self {
            frontend,
            armed: true,
        };
        guard
            .emit_best_effort(AgentEventPayload::CompactProgress { phase, detail })
            .await;
        guard
    }

    pub(super) async fn finish(mut self, detail: Option<String>) {
        let payload = AgentEventPayload::CompactProgress {
            phase: CompactPhase::Done,
            detail,
        };
        match self.frontend.emit(payload).await {
            Ok(()) => self.armed = false,
            Err(e) => {
                tracing::warn!(error = %e, "compact terminal progress emit dropped");
            }
        }
    }

    async fn emit_best_effort(&self, payload: AgentEventPayload) {
        if let Err(e) = self.frontend.emit(payload).await {
            tracing::warn!(error = %e, "compact progress emit dropped");
        }
    }
}

impl Drop for CompactProgressGuard {
    fn drop(&mut self) {
        if self.armed
            && !self.frontend.try_emit(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Done,
                detail: Some("compaction interrupted".to_string()),
            })
        {
            tracing::warn!("compact lifecycle ended without a terminal progress event");
        }
    }
}
