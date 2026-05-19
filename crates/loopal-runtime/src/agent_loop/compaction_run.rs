//! Compaction execution: invoke summarizer, persist atomically, emit
//! lifecycle events. Split from `compaction.rs` so the public entry
//! points there (auto-trigger, force-trigger, microcompact) stay short
//! and the failure-semantics commentary lives next to the code it
//! describes.

use loopal_context::middleware::smart_compact::{CompactOutput, compact_to_boundary};
use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, CompactPhase};
use tracing::{info, warn};

use super::runner::AgentLoopRunner;

#[derive(Debug, Clone, Copy)]
pub(super) enum CompactTrigger {
    Auto,
    Manual,
}

impl CompactTrigger {
    fn label(self) -> &'static str {
        match self {
            CompactTrigger::Auto => "auto",
            CompactTrigger::Manual => "manual",
        }
    }
}

#[derive(Debug)]
pub(super) struct PersistResult {
    pub summary_msg_id: Option<String>,
    pub files_rehydrated: usize,
}

impl AgentLoopRunner {
    pub(super) async fn run_smart_compact(
        &mut self,
        before_count: usize,
        tokens_before: u32,
        instructions: Option<String>,
        trigger: CompactTrigger,
    ) -> Result<()> {
        let compact_model = self
            .params
            .config
            .router
            .resolve(loopal_provider_api::TaskType::Summarization)
            .to_string();
        let provider = match self.params.deps.kernel.resolve_provider(&compact_model) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, model = %compact_model, "summarization provider unavailable");
                return Ok(());
            }
        };

        let boundary_at = self.params.store.compact_boundary_at();

        let result = compact_to_boundary(
            self.params.store.messages(),
            &*provider,
            &compact_model,
            boundary_at,
            instructions.as_deref(),
        )
        .await;

        let Some(output) = (match result {
            Ok(opt) => opt,
            Err(e) => {
                warn!(error = %e, "compaction LLM call failed");
                return Ok(());
            }
        }) else {
            return Ok(());
        };

        let apply_result = self.persist_and_apply(output, boundary_at).await?;
        self.post_compact(
            before_count,
            tokens_before,
            trigger,
            apply_result.summary_msg_id,
            apply_result.files_rehydrated,
        )
        .await
    }

    /// Persist the summary/ack pair and anchor the boundary marker.
    ///
    /// Failure semantics (read this before touching the order):
    /// * `save_message(summary)` fails — caller propagates; nothing else
    ///   has run, no on-disk state, store untouched. Caller will retry on
    ///   the next compact tick.
    /// * `save_message(ack)` fails after summary saved — propagated; the
    ///   summary message lives on disk without a boundary marker. Replay
    ///   falls back to the safe path (keep full history) because the
    ///   marker is the anchor; the orphan summary message is harmless.
    /// * `mark_compact_boundary` fails — propagated; same safe fallback.
    /// * `store.set_boundary` is the last step and is infallible; it only
    ///   runs after every persistence step has succeeded, which keeps
    ///   in-memory and on-disk views consistent.
    /// * `compact_rehydrate` is best-effort; its failure is logged but
    ///   never bubbled up so we don't undo a successful boundary commit.
    async fn persist_and_apply(
        &mut self,
        output: CompactOutput,
        boundary_at: usize,
    ) -> Result<PersistResult> {
        let CompactOutput {
            mut summary_msg,
            mut ack_msg,
            touched_files,
            ..
        } = output;

        self.params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut summary_msg)?;
        self.params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut ack_msg)?;

        let summary_id = summary_msg.id.clone().expect("save_message assigns a UUID");
        self.params
            .deps
            .session_manager
            .mark_compact_boundary(&self.params.session.id, &summary_id)?;

        self.params
            .store
            .set_boundary(boundary_at, summary_msg, ack_msg);

        if !touched_files.is_empty() {
            self.emit(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Rehydrate,
                detail: Some(format!("re-reading {} files", touched_files.len().min(5))),
            })
            .await?;
        }
        let rehydrate = self.compact_rehydrate(&touched_files).await;
        Ok(PersistResult {
            summary_msg_id: Some(summary_id),
            files_rehydrated: rehydrate.files_succeeded,
        })
    }

    async fn post_compact(
        &mut self,
        before: usize,
        tokens_before: u32,
        trigger: CompactTrigger,
        summary_msg_id: Option<String>,
        files_rehydrated: usize,
    ) -> Result<()> {
        let after = self.params.store.len();
        let removed = before.saturating_sub(after);
        let tokens_after = self.params.store.current_tokens();
        let strategy = trigger.label();

        self.emit(AgentEventPayload::Compacted(
            loopal_protocol::CompactionSummary {
                kept: after,
                removed,
                tokens_before,
                tokens_after,
                strategy: strategy.to_string(),
                summary_msg_id,
                files_rehydrated,
            },
        ))
        .await?;

        // Done phase closes the progress stream so frontends can collapse
        // the inline indicator. Emitted last, after `Compacted` so any
        // listener that wants the final stats has them in hand.
        self.emit(AgentEventPayload::CompactProgress {
            phase: CompactPhase::Done,
            detail: None,
        })
        .await?;

        info!(
            before,
            after, removed, tokens_before, tokens_after, strategy, "compaction complete"
        );
        Ok(())
    }
}
