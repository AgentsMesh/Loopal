use super::ContextStore;
use crate::degradation::run_sync_degradation;
use crate::token_counter::estimate_messages_tokens;
use loopal_provider_api::Message;
use tracing::debug;

impl ContextStore {
    pub fn prepare_for_llm(&self) -> Vec<Message> {
        self.messages().to_vec()
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
        use crate::ingestion::{cap_assistant_server_blocks, cap_tool_results};
        use loopal_provider_api::MessageRole;
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
}
