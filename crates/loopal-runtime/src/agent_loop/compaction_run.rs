use std::sync::Arc;

use async_trait::async_trait;
use loopal_context::middleware::smart_compact::{
    CompactOutput, CompactRetryEvent, CompactRetryObserver, compact_to_boundary_observed,
};
use loopal_error::{LoopalError, Result};
use loopal_protocol::{AgentEventPayload, CompactPhase};
use loopal_turn::{CompactionSummary, TurnStep};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::frontend::traits::AgentFrontend;

use super::runner::AgentLoopRunner;

struct FrontendCompactRetryObserver {
    frontend: Arc<dyn AgentFrontend>,
}

#[async_trait]
impl CompactRetryObserver for FrontendCompactRetryObserver {
    async fn observe(&self, event: CompactRetryEvent) {
        let payload = match event {
            CompactRetryEvent::Scheduled {
                error,
                attempt,
                max_retries,
                wait,
            } => AgentEventPayload::RetryError {
                message: format!(
                    "Compaction: {error}. Retrying in {:.1}s",
                    wait.as_secs_f64()
                ),
                attempt,
                max_attempts: max_retries,
            },
            CompactRetryEvent::Succeeded { retries }
            | CompactRetryEvent::Exhausted { retries }
            | CompactRetryEvent::Failed { retries }
            | CompactRetryEvent::Cancelled { retries } => {
                if retries == 0 {
                    return;
                }
                AgentEventPayload::RetryCleared
            }
        };
        if let Err(e) = self.frontend.emit(payload).await {
            tracing::warn!(error = %e, "compaction retry state emit dropped");
        }
    }
}

#[derive(Debug)]
pub(super) struct PersistResult {
    pub summary_msg_id: Option<String>,
    pub files_rehydrated: usize,
}

fn cancelled_before_summary_commit() -> LoopalError {
    LoopalError::Other("compaction cancelled before summary commit".into())
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
                // Non-terminal system notice. `Stream` is reserved for model
                // output and would incorrectly switch an idle manual compact
                // to Running while also clearing its compact banner.
                self.emit_cosmetic(AgentEventPayload::ProviderWarning {
                    message: format!(
                        "Compaction unavailable: summarization model has no provider ({e}); \
                         set model_routing.summarization or configure the provider."
                    ),
                })
                .await;
                return Ok(false);
            }
        };

        let boundary_turn_at = self.turns.store().compact_boundary_turn_index();

        let retry_observer = FrontendCompactRetryObserver {
            frontend: Arc::clone(&self.params.deps.frontend),
        };

        let result = compact_to_boundary_observed(
            self.turns.store().turns(),
            &*provider,
            &compact_model,
            boundary_turn_at,
            instructions.as_deref(),
            cancel,
            &retry_observer,
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
        .await;
        Ok(true)
    }

    async fn persist_and_apply(
        &mut self,
        output: CompactOutput,
        cancel: &CancellationToken,
    ) -> Result<PersistResult> {
        if cancel.is_cancelled() {
            return Err(cancelled_before_summary_commit());
        }
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
            Some(p) => {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        return Err(cancelled_before_summary_commit());
                    }
                    digest = p.outstanding_tasks_digest() => digest,
                }
            }
            None => None,
        };
        if let Some(digest) = task_digest {
            summary_text.push_str(&digest);
        }

        // Linearization point: once this final check passes, the synchronous
        // append below owns the race and the summary is committed. Everything
        // after append is best-effort post-commit work and must not roll back.
        if cancel.is_cancelled() {
            return Err(cancelled_before_summary_commit());
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
            self.emit_cosmetic(AgentEventPayload::CompactProgress {
                phase: CompactPhase::Rehydrate,
                detail: Some(format!("re-reading {} files", touched_files.len().min(5))),
            })
            .await;
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
    ) {
        let after = self.turns.view().len();
        let summarized = before.saturating_sub(after);
        let tokens_after = self.turns.view().current_tokens();

        // The summary is already durable at this point. Delivery failures
        // cannot turn a committed compaction back into a no-op; clients that
        // reconnect rebuild from the persisted turn history.
        self.emit_cosmetic(AgentEventPayload::Compacted(
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
        .await;

        // Route the post-compact token truth through the canonical token path
        // so the status bar's ctx counter refreshes; mirrors resume reset.
        self.emit_cosmetic(AgentEventPayload::TokenUsage {
            input_tokens: tokens_after,
            output_tokens: 0,
            context_window: self.turns.view().budget().context_window,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        })
        .await;

        info!(
            before,
            after, summarized, tokens_before, tokens_after, strategy, "compaction complete"
        );
    }
}
