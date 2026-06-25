use loopal_context::middleware::smart_compact::{CompactOutput, compact_to_boundary};
use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, CompactPhase};
use loopal_turn::{CompactionSummary, TurnStep};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::runner::AgentLoopRunner;

#[derive(Debug)]
pub(super) struct PersistResult {
    pub summary_msg_id: Option<String>,
    pub files_rehydrated: usize,
}

impl AgentLoopRunner {
    /// Run a smart compaction pass. Returns `true` iff a `CompactionSummary`
    /// step was actually appended; `false` for guarded silent fall-throughs
    /// (provider unresolved, compact LLM Err, compact_to_boundary Ok(None)).
    pub(super) async fn run_smart_compact(
        &mut self,
        before_count: usize,
        tokens_before: u32,
        instructions: Option<String>,
        strategy: &'static str,
        cancel: &CancellationToken,
    ) -> Result<bool> {
        let resolved = {
            let router = self.params.config.router.read();
            self.params
                .deps
                .kernel
                .resolve_task(&router, loopal_provider_api::TaskType::Summarization)
        };
        let (compact_model, provider) = match resolved {
            Ok(pair) => pair,
            Err(e) => {
                warn!(error = %e, "summarization provider unavailable");
                // Inline notice, best-effort — NOT AgentEventPayload::Error,
                // which frontends treat as agent termination (snapping a viewed
                // sub-agent back to root every auto-compact tick).
                self.emit_cosmetic(AgentEventPayload::Stream {
                    text: format!(
                        "[compaction unavailable: summarization model has no provider ({e}); \
                         set model_routing.summarization or configure the provider]\n"
                    ),
                })
                .await;
                return Ok(false);
            }
        };

        let boundary_turn_at = self.turns.store().compact_boundary_turn_index();

        let result = compact_to_boundary(
            self.turns.store().turns(),
            &*provider,
            &compact_model,
            boundary_turn_at,
            instructions.as_deref(),
            cancel,
        )
        .await;

        let Some(output) = (match result {
            Ok(opt) => opt,
            Err(e) => {
                warn!(error = %e, "compaction LLM call failed");
                return Ok(false);
            }
        }) else {
            return Ok(false);
        };

        let apply_result = match self.persist_and_apply(output, cancel).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "persist_and_apply failed; reporting compact as no-op");
                return Ok(false);
            }
        };
        self.post_compact(
            before_count,
            tokens_before,
            strategy,
            apply_result.summary_msg_id,
            apply_result.files_rehydrated,
        )
        .await?;
        Ok(true)
    }

    async fn persist_and_apply(
        &mut self,
        output: CompactOutput,
        cancel: &CancellationToken,
    ) -> Result<PersistResult> {
        let CompactOutput {
            summary_msg,
            ack_msg,
            touched_files,
            removed_turn_count,
            kept_turn_count,
            ..
        } = output;

        // Carry the not-yet-completed task list across the compaction boundary
        // (TaskCreate/TaskUpdate records are otherwise summarized away, making
        // the agent forget its in-progress/pending work).
        let mut summary_text = summary_msg.text_content();
        let task_digest = match &self.params.outstanding_tasks {
            Some(p) => p.outstanding_tasks_digest().await,
            None => None,
        };
        if let Some(digest) = task_digest {
            summary_text.push_str(&digest);
        }

        if let Err(e) = self.append_step_record(TurnStep::CompactionSummary(CompactionSummary {
            summary_text,
            ack_text: ack_msg.text_content(),
            kept_turn_count,
            removed_turn_count,
        })) {
            warn!(error = %e, "append_step(CompactionSummary) failed");
            return Err(loopal_error::LoopalError::Other(format!(
                "compaction summary append failed: {e}"
            )));
        }

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
            summary_msg_id: None,
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
        let after = self.turns.view().len();
        let summarized = before.saturating_sub(after);
        let tokens_after = self.turns.view().current_tokens();

        self.emit(AgentEventPayload::Compacted(
            loopal_protocol::CompactionSummary {
                kept: after,
                summarized,
                tokens_before,
                tokens_after,
                strategy: strategy.to_string(),
                summary_msg_id,
                files_rehydrated,
            },
        ))
        .await?;

        // Route the post-compact token truth through the canonical token path
        // so the status bar's ctx counter refreshes; mirrors resume reset.
        self.emit(AgentEventPayload::TokenUsage {
            input_tokens: tokens_after,
            output_tokens: 0,
            context_window: self.turns.view().budget().context_window,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        })
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
            after, summarized, tokens_before, tokens_after, strategy, "compaction complete"
        );
        Ok(())
    }
}
