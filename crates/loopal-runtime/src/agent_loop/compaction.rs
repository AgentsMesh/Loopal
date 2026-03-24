//! Persistent compaction — LLM summarization + emergency truncation.
//!
//! The ContextStore handles sync degradation (strip, truncate) automatically.
//! This module handles the async Layer 2 (LLM summarization) that the store
//! cannot do on its own (requires Provider access).

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use tracing::{info, warn};

use super::runner::AgentLoopRunner;

/// Minimum messages to keep during emergency compaction.
const EMERGENCY_KEEP_LAST: usize = 5;

impl AgentLoopRunner {
    /// Check if LLM summarization is needed and apply it.
    ///
    /// The ContextStore's sync degradation (layers 0, 1, 3) already runs on
    /// every push. This method handles Layer 2: async LLM summarization
    /// when messages exceed 75% of budget.
    pub async fn check_and_compact(&mut self) -> Result<()> {
        if !self.params.store.needs_summarization() {
            return Ok(());
        }

        let msg_tokens = self.params.store.current_tokens();
        let budget = self.params.store.budget().clone();

        info!(
            msg_tokens,
            message_budget = budget.message_budget,
            messages = self.params.store.len(),
            "compaction triggered"
        );

        self.emit(AgentEventPayload::Stream {
            text: "[compacting context...]\n".to_string(),
        })
        .await?;

        let before = self.params.store.len();
        let tokens_before = msg_tokens;

        // Try LLM summarization first, fall back to emergency truncation
        if !budget.needs_emergency(msg_tokens) {
            if self.try_smart_compact().await {
                self.post_compact(before, tokens_before, "smart").await?;
                return Ok(());
            }
            warn!("smart compact failed, falling back to emergency truncation");
        }

        self.params.store.emergency_compact(EMERGENCY_KEEP_LAST);
        self.post_compact(before, tokens_before, "emergency")
            .await?;
        Ok(())
    }

    /// Attempt LLM-based summarization. Returns true if successful.
    async fn try_smart_compact(&mut self) -> bool {
        let compact_model = self
            .params
            .compact_model
            .as_deref()
            .unwrap_or(&self.params.model);
        let Ok(provider) = self.params.kernel.resolve_provider(compact_model) else {
            warn!("no summarization provider available");
            return false;
        };

        let keep_last = self.params.store.token_aware_keep_count();
        let result = loopal_context::middleware::smart_compact::summarize_old_messages(
            self.params.store.messages(), // &[Message] — read only
            &*provider,
            compact_model,
            keep_last,
        )
        .await;

        match result {
            Ok(Some(new_messages)) => {
                // apply_summary validates tokens, reverts if inflated
                if self.params.store.apply_summary(new_messages) {
                    true
                } else {
                    warn!("summary inflated tokens, reverted");
                    false
                }
            }
            Ok(None) => false,
            Err(e) => {
                warn!(error = %e, "summarization failed");
                false
            }
        }
    }

    /// Force compaction unconditionally (user-triggered `/compact`).
    pub async fn force_compact(&mut self) -> Result<()> {
        let before = self.params.store.len();
        let keep_last = self.params.store.token_aware_keep_count();
        if before <= keep_last {
            self.emit(AgentEventPayload::Stream {
                text: "[nothing to compact — conversation is short]\n".to_string(),
            })
            .await?;
            return Ok(());
        }
        let tokens_before = self.params.store.current_tokens();

        self.emit(AgentEventPayload::Stream {
            text: "[compacting context...]\n".to_string(),
        })
        .await?;

        info!(
            tokens_before,
            messages = before,
            "manual compaction triggered"
        );

        if self.try_smart_compact().await {
            self.post_compact(before, tokens_before, "manual-smart")
                .await?;
        } else {
            warn!("smart compact failed for manual /compact, falling back");
            self.params.store.emergency_compact(keep_last);
            self.post_compact(before, tokens_before, "manual-emergency")
                .await?;
        }
        Ok(())
    }

    /// Post-compaction: write marker, emit event with metrics.
    async fn post_compact(
        &mut self,
        before: usize,
        tokens_before: u32,
        strategy: &str,
    ) -> Result<()> {
        let after = self.params.store.len();
        let removed = before.saturating_sub(after);
        let tokens_after = self.params.store.current_tokens();

        if let Err(e) = self
            .params
            .session_manager
            .compact_history(&self.params.session.id, after)
        {
            warn!(error = %e, "failed to write compact marker");
        }

        self.emit(AgentEventPayload::Compacted {
            kept: after,
            removed,
            tokens_before,
            tokens_after,
            strategy: strategy.to_string(),
        })
        .await?;

        info!(
            before,
            after, removed, tokens_before, tokens_after, strategy, "compaction complete"
        );
        Ok(())
    }
}
