//! Persistent compaction — modifies params.messages in place + writes CompactTo marker.

use loopal_context::budget::ContextBudget;
use loopal_context::compaction::{compact_messages, sanitize_tool_pairs};
use loopal_context::token_counter::estimate_messages_tokens;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use tracing::{info, warn};

use super::runner::AgentLoopRunner;

/// Number of recent messages to keep during smart compaction.
const SMART_COMPACT_KEEP_LAST: usize = 10;
/// Number of recent messages to keep during emergency compaction.
const EMERGENCY_KEEP_LAST: usize = 5;

impl AgentLoopRunner {
    /// Check if compaction is needed and apply it persistently.
    ///
    /// This is a **lifecycle event**, not a per-turn filter:
    /// - Computes a precise budget (subtracting system prompt, tools, output reserve)
    /// - If messages exceed 75% of budget → LLM summarization (smart compact)
    /// - If messages exceed 95% of budget → emergency truncation
    /// - Writes a CompactTo marker to disk for session reload consistency
    pub async fn check_and_compact(&mut self) -> Result<()> {
        let tool_defs = self.params.kernel.tool_definitions();
        let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
        let budget = ContextBudget::calculate(
            self.model_config.max_context_tokens,
            &self.params.system_prompt,
            tool_tokens,
            self.model_config.max_output_tokens,
        );
        let msg_tokens = estimate_messages_tokens(&self.params.messages);

        if !budget.needs_compaction(msg_tokens) {
            return Ok(());
        }

        info!(
            msg_tokens,
            message_budget = budget.message_budget,
            messages = self.params.messages.len(),
            "compaction triggered"
        );

        // Notify user that compaction is starting (LLM summarization may take seconds)
        self.emit(AgentEventPayload::Stream {
            text: "[compacting context...]\n".to_string(),
        })
        .await?;

        let before = self.params.messages.len();
        let tokens_before = msg_tokens;

        // Try LLM summarization first, fall back to emergency truncation
        if !budget.needs_emergency(msg_tokens) {
            if self.try_smart_compact(&budget).await {
                self.post_compact(before, tokens_before, "smart").await?;
                return Ok(());
            }
            // Smart compact failed — fall through to emergency
            warn!("smart compact failed, falling back to emergency truncation");
        }

        compact_messages(&mut self.params.messages, EMERGENCY_KEEP_LAST);
        self.post_compact(before, tokens_before, "emergency")
            .await?;
        Ok(())
    }

    /// Attempt LLM-based summarization. Returns true if successful.
    /// On failure or inflation, params.messages is reverted to pre-summarization state.
    async fn try_smart_compact(&mut self, budget: &ContextBudget) -> bool {
        let compact_model = self
            .params
            .compact_model
            .as_deref()
            .unwrap_or(&self.params.model);
        let Ok(provider) = self.params.kernel.resolve_provider(compact_model) else {
            warn!("no summarization provider available");
            return false;
        };

        // Snapshot before mutation — revert if summarization produces bad results
        let snapshot = self.params.messages.clone();
        let keep_last = SMART_COMPACT_KEEP_LAST.min(self.params.messages.len());
        let result = loopal_context::middleware::smart_compact::summarize_old_messages(
            &mut self.params.messages,
            &*provider,
            compact_model,
            keep_last,
        )
        .await;

        match result {
            Ok(true) => {
                // Validate: if summary inflated tokens, revert to snapshot
                let post_tokens = estimate_messages_tokens(&self.params.messages);
                if budget.needs_emergency(post_tokens) {
                    warn!(post_tokens, "summary inflated tokens, reverting");
                    self.params.messages = snapshot;
                    return false;
                }
                true
            }
            Ok(false) => false,
            Err(e) => {
                warn!(error = %e, "summarization failed");
                self.params.messages = snapshot;
                false
            }
        }
    }

    /// Force compaction unconditionally (user-triggered `/compact`).
    /// Tries LLM summarization first, falls back to blind truncation.
    pub async fn force_compact(&mut self) -> Result<()> {
        let before = self.params.messages.len();
        if before <= SMART_COMPACT_KEEP_LAST {
            self.emit(AgentEventPayload::Stream {
                text: "[nothing to compact — conversation is short]\n".to_string(),
            })
            .await?;
            return Ok(());
        }
        let tokens_before = estimate_messages_tokens(&self.params.messages);

        self.emit(AgentEventPayload::Stream {
            text: "[compacting context...]\n".to_string(),
        })
        .await?;

        info!(
            tokens_before,
            messages = before,
            "manual compaction triggered"
        );

        let tool_defs = self.params.kernel.tool_definitions();
        let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
        let budget = ContextBudget::calculate(
            self.model_config.max_context_tokens,
            &self.params.system_prompt,
            tool_tokens,
            self.model_config.max_output_tokens,
        );

        if self.try_smart_compact(&budget).await {
            self.post_compact(before, tokens_before, "manual-smart")
                .await?;
        } else {
            warn!("smart compact failed for manual /compact, falling back to truncation");
            compact_messages(&mut self.params.messages, SMART_COMPACT_KEEP_LAST);
            self.post_compact(before, tokens_before, "manual-emergency")
                .await?;
        }
        Ok(())
    }

    /// Post-compaction: sanitize pairs, write marker, emit event with metrics.
    async fn post_compact(
        &mut self,
        before: usize,
        tokens_before: u32,
        strategy: &str,
    ) -> Result<()> {
        sanitize_tool_pairs(&mut self.params.messages);

        let after = self.params.messages.len();
        let removed = before.saturating_sub(after);
        let tokens_after = estimate_messages_tokens(&self.params.messages);

        // Write marker to disk for session reload consistency.
        // `after` is the new total message count (including any system messages);
        // on reload, replay keeps the last `after` entries from the JSONL log.
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
