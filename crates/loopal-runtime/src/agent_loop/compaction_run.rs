use loopal_context::middleware::smart_compact::{CompactOutput, compact_to_boundary};
use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, CompactPhase};
use loopal_turn::{CompactionRecord, TurnStep};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::runner::AgentLoopRunner;

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
        strategy: &'static str,
        cancel: &CancellationToken,
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
            cancel,
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

        let apply_result = self.persist_and_apply(output, boundary_at, cancel).await?;
        self.post_compact(
            before_count,
            tokens_before,
            strategy,
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
    ///   It also honors `cancel` so a mid-rehydrate interrupt cannot leave
    ///   an orphan `ToolUse` block in the store.
    async fn persist_and_apply(
        &mut self,
        output: CompactOutput,
        boundary_at: usize,
        cancel: &CancellationToken,
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

        // Domain mirror: record one Compaction step with both summary + ack texts.
        let removed_count = boundary_at as u32;
        self.append_step_record(TurnStep::Compaction(CompactionRecord {
            summary_text: summary_msg.text_content(),
            ack_text: ack_msg.text_content(),
            rehydrated: vec![],
            kept_turn_count: 0,
            removed_turn_count: removed_count,
        }));

        let summary_id = summary_msg.id.clone().expect("save_message assigns a UUID");
        self.params
            .deps
            .session_manager
            .mark_compact_boundary(&self.params.session.id, &summary_id)?;

        self.params
            .store
            .set_boundary(boundary_at, summary_msg, ack_msg);

        // Compaction rewrote earlier history. Notify cross-turn governance
        // state (LoopDetector signatures, etc.) so they don't carry stale
        // counts derived from messages no longer in the store.
        for g in self.governance.iter_mut() {
            g.on_compact_completed();
        }

        if !touched_files.is_empty() {
            self.emit(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Rehydrate,
                detail: Some(format!("re-reading {} files", touched_files.len().min(5))),
            })
            .await?;
        }
        let rehydrate = self.compact_rehydrate(&touched_files, cancel).await;
        Ok(PersistResult {
            summary_msg_id: Some(summary_id),
            files_rehydrated: rehydrate.files_succeeded,
        })
    }

    async fn post_compact(
        &mut self,
        before: usize,
        tokens_before: u32,
        strategy: &'static str,
        summary_msg_id: Option<String>,
        files_rehydrated: usize,
    ) -> Result<()> {
        let after = self.params.store.len();
        let removed = before.saturating_sub(after);
        let tokens_after = self.params.store.current_tokens();

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
