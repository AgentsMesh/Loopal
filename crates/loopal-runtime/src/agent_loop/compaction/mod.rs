mod host;

use std::time::{Duration, SystemTime};

use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, CompactPhase};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, warn};

use self::host::CompactionHostGuard;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub async fn check_and_microcompact(&mut self) -> Result<()> {
        let idle = self.params.config.microcompact_idle;
        if idle == Duration::ZERO {
            return Ok(());
        }
        let last = self.turns.store().last_assistant_activity_at();
        let now = SystemTime::now();
        let stats = match self
            .turns
            .with_wire_mut(|turns| loopal_context::scrub_idle_tool_results(turns, last, now, idle))
        {
            Some(s) if s.results_cleared > 0 => s,
            _ => return Ok(()),
        };
        info!(
            cleared = stats.results_cleared,
            idle_seconds = idle.as_secs(),
            "microcompact scrubbed old tool results"
        );
        self.emit_cosmetic(AgentEventPayload::CompactProgress {
            phase: CompactPhase::Microcompact,
            detail: Some(format!(
                "scrubbed {} stale tool results",
                stats.results_cleared
            )),
        })
        .await;
        Ok(())
    }

    pub async fn check_and_compact(&mut self, cancel: &CancellationToken) -> Result<()> {
        let compact_span = tracing::info_span!("context_compact");
        async {
            if !self.turns.view().needs_summarization() {
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

            let tokens_before = self.turns.view().effective_tokens();
            let before_count = self.turns.view().len();

            info!(
                tokens_before,
                context_window = self.turns.view().budget().context_window,
                messages = before_count,
                "auto compaction triggered"
            );

            self.emit_cosmetic(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Summarize,
                detail: Some(format!("{tokens_before} tokens before")),
            })
            .await;

            let compacted = self
                .run_smart_compact(before_count, tokens_before, None, "auto", cancel)
                .await?;
            if !compacted {
                self.emit_cosmetic(AgentEventPayload::CompactProgress {
                    phase: CompactPhase::Done,
                    detail: Some("auto-compact declined: no summary produced".to_string()),
                })
                .await;
                warn!("auto-compact run_smart_compact returned false");
            }
            Ok(())
        }
        .instrument(compact_span)
        .await
    }

    /// Force a compaction pass. Returns `true` iff a `CompactionSummary` was
    /// actually appended; `false` means the call was a guarded no-op.
    /// ContextOverflow recovery MUST check the return value — blindly retrying
    /// after a `false` loops against unchanged state.
    pub async fn force_compact(&mut self, instructions: Option<String>) -> Result<bool> {
        let turn_count = self.turns.store().turns().len();
        let in_progress_offset = if self.turns.current_turn_id().is_some() {
            1
        } else {
            0
        };
        let compactable_turns = turn_count.saturating_sub(in_progress_offset);
        if compactable_turns < 2 {
            self.emit_cosmetic(AgentEventPayload::Stream {
                text: "[nothing to compact — conversation is short]\n".to_string(),
            })
            .await;
            self.emit_cosmetic(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Done,
                detail: None,
            })
            .await;
            return Ok(false);
        }
        let before_count = self.turns.view().len();

        crate::fire_hooks::fire_hooks(
            &self.params.deps.kernel,
            loopal_config::HookEvent::PreCompact,
            &loopal_hooks::HookContext {
                session_id: Some(&self.params.session.id),
                ..Default::default()
            },
        )
        .await;

        let tokens_before = self.turns.view().effective_tokens();

        info!(
            tokens_before,
            messages = before_count,
            "manual compaction triggered"
        );

        self.emit_cosmetic(AgentEventPayload::CompactProgress {
            phase: CompactPhase::Summarize,
            detail: Some(format!("{tokens_before} tokens before")),
        })
        .await;

        let host = self.ensure_compaction_host_turn();
        if matches!(host, CompactionHostGuard::OpenFailed) {
            warn!("force_compact: failed to open synthetic host turn; aborting");
            self.emit_cosmetic(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Done,
                detail: Some("aborted: cannot persist host turn".to_string()),
            })
            .await;
            return Ok(false);
        }
        if matches!(host, CompactionHostGuard::AlreadySummarized) {
            self.emit_cosmetic(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Done,
                detail: Some("nothing new to compact since last summary".to_string()),
            })
            .await;
            return Ok(false);
        }

        let cancel =
            super::cancel::TurnCancel::new(self.interrupt.clone(), self.interrupt_tx.clone());
        let result = self
            .run_smart_compact(
                before_count,
                tokens_before,
                instructions,
                "manual",
                cancel.token(),
            )
            .await;

        let summary_appended = matches!(result, Ok(true));
        if matches!(result, Ok(false)) {
            self.emit_cosmetic(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Done,
                detail: Some("no summary produced".to_string()),
            })
            .await;
        }

        self.close_compaction_host(host, summary_appended);
        result
    }
}
