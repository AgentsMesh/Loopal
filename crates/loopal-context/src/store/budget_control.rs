use super::ContextStore;
use crate::compaction::sanitize_tool_pairs;
use crate::degradation::run_sync_degradation;
use crate::ingestion::{cap_assistant_server_blocks, cap_tool_results};
use crate::token_counter::estimate_messages_tokens;
use loopal_provider_api::{Message, MessageRole};
use tracing::debug;

impl ContextStore {
    pub fn prepare_for_llm(&self) -> Vec<Message> {
        self.messages().to_vec()
    }

    /// Replace the segment `[..boundary_at]` with a `[summary, ack]` prefix.
    /// The caller is responsible for persisting the two messages and writing
    /// the `Marker::CompactBoundary` anchor — this only mutates the in-memory
    /// view used to build the next LLM request.
    pub fn set_boundary(&mut self, boundary_at: usize, summary: Message, ack: Message) {
        let kept = self.messages().get(boundary_at..).unwrap_or(&[]);
        let mut new_msgs = Vec::with_capacity(kept.len() + 2);
        new_msgs.push(summary);
        new_msgs.push(ack);
        new_msgs.extend_from_slice(kept);
        self.replace_messages(new_msgs);
        self.sanitize();
        self.enforce_budget();
    }

    pub fn condense_server_blocks(&mut self) {
        crate::ingestion::condense_all_server_blocks(self.messages_mut());
    }

    pub fn needs_summarization(&self) -> bool {
        self.budget().needs_compaction(self.effective_tokens())
    }

    /// Pick the cut point at which compaction summarizes everything *before*
    /// and preserves everything *after*. The current rule is "keep the last
    /// two messages" (`saturating_sub(2).max(1)`) so the model continues
    /// from the most recent turn without losing the active user request.
    /// Living here, not in the runtime, keeps the boundary rule attached to
    /// the data it operates on (GRASP Information Expert).
    pub fn compact_boundary_at(&self) -> usize {
        const KEEP_TAIL: usize = 2;
        self.len().saturating_sub(KEEP_TAIL).max(1)
    }

    pub fn current_tokens(&self) -> u32 {
        estimate_messages_tokens(self.messages())
    }

    /// Single source of truth for "how many tokens does the upcoming request
    /// actually weigh." Combines the local BPE estimate with the most recent
    /// provider-reported `input_tokens` (monotonically non-decreasing).
    pub fn effective_tokens(&self) -> u32 {
        let estimate = self.current_tokens();
        match self.last_actual_input_tokens() {
            Some(actual) => estimate.max(actual),
            None => estimate,
        }
    }

    pub(super) fn enforce_budget(&mut self) {
        let budget = self.budget().clone();
        run_sync_degradation(self.messages_mut(), &budget);
        debug!(
            tokens = estimate_messages_tokens(self.messages()),
            budget = budget.message_budget,
            messages = self.messages().len(),
            "budget enforced"
        );
    }

    pub(super) fn apply_ingestion_caps(&mut self) {
        let max_server = self.budget().message_budget / 4;
        let max_result = self.budget().message_budget / 8;
        for msg in self.messages_mut() {
            if msg.role == MessageRole::Assistant {
                cap_assistant_server_blocks(msg, max_server);
            } else if msg.role == MessageRole::User {
                cap_tool_results(msg, max_result);
            }
        }
    }

    fn sanitize(&mut self) {
        sanitize_tool_pairs(self.messages_mut());
    }
}
