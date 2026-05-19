//! Per-turn compaction triggers.
//!
//! Three entry points, all narrow:
//!   * `check_and_microcompact` — free idle-time scrub of old tool results
//!   * `check_and_compact` — auto-trigger when input crosses the budget
//!   * `force_compact` — user-initiated `/compact`
//!
//! Execution detail (LLM call, persistence, event emission) lives in
//! `compaction_run.rs` so this file stays a thin dispatch layer.

use std::time::{Duration, SystemTime};

use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, CompactPhase};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info};

use super::compaction_run::CompactTrigger;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub async fn check_and_microcompact(&mut self) -> Result<()> {
        let idle = self.params.config.microcompact_idle;
        if idle == Duration::ZERO {
            return Ok(());
        }
        let last = self.params.store.last_assistant_activity_at();
        let now = SystemTime::now();
        let stats = match self.params.store.apply_microcompact(last, now, idle) {
            Some(s) if s.results_cleared > 0 => s,
            _ => return Ok(()),
        };
        info!(
            cleared = stats.results_cleared,
            idle_seconds = idle.as_secs(),
            "microcompact scrubbed old tool results"
        );
        self.params.store.record_assistant_activity(now);
        self.emit(AgentEventPayload::CompactProgress {
            phase: CompactPhase::Microcompact,
            detail: Some(format!(
                "scrubbed {} stale tool results",
                stats.results_cleared
            )),
        })
        .await?;
        Ok(())
    }

    pub async fn check_and_compact(&mut self, cancel: &CancellationToken) -> Result<()> {
        let compact_span = tracing::info_span!("context_compact");
        async {
            if !self.params.store.needs_summarization() {
                return Ok(());
            }

            crate::fire_hooks::fire_hooks(
                &self.params.deps.kernel,
                loopal_config::HookEvent::PreCompact,
                &loopal_hooks::HookContext {
                    session_id: Some(&self.params.session.id),
                    ..Default::default()
                },
            )
            .await;

            let tokens_before = self.params.store.effective_tokens();
            let before_count = self.params.store.len();

            info!(
                tokens_before,
                context_window = self.params.store.budget().context_window,
                messages = before_count,
                "auto compaction triggered"
            );

            self.emit(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Summarize,
                detail: Some(format!("{tokens_before} tokens before")),
            })
            .await?;

            self.run_smart_compact(
                before_count,
                tokens_before,
                None,
                CompactTrigger::Auto,
                cancel,
            )
            .await
        }
        .instrument(compact_span)
        .await
    }

    pub async fn force_compact(&mut self, instructions: Option<String>) -> Result<()> {
        let before_count = self.params.store.len();
        if before_count <= 2 {
            self.emit(AgentEventPayload::Stream {
                text: "[nothing to compact — conversation is short]\n".to_string(),
            })
            .await?;
            return Ok(());
        }

        crate::fire_hooks::fire_hooks(
            &self.params.deps.kernel,
            loopal_config::HookEvent::PreCompact,
            &loopal_hooks::HookContext {
                session_id: Some(&self.params.session.id),
                ..Default::default()
            },
        )
        .await;

        let tokens_before = self.params.store.effective_tokens();

        info!(
            tokens_before,
            messages = before_count,
            "manual compaction triggered"
        );

        self.emit(AgentEventPayload::CompactProgress {
            phase: CompactPhase::Summarize,
            detail: Some(format!("{tokens_before} tokens before")),
        })
        .await?;

        // Bridge the cross-boundary `InterruptSignal` into a fresh
        // `CancellationToken`. `/compact` (and the `force_compact` retry
        // path triggered by ContextOverflow recovery) runs outside any
        // active turn, so there is no `TurnCancel` to borrow.
        let cancel =
            super::cancel::TurnCancel::new(self.interrupt.clone(), self.interrupt_tx.clone());
        self.run_smart_compact(
            before_count,
            tokens_before,
            instructions,
            CompactTrigger::Manual,
            cancel.token(),
        )
        .await
    }
}
