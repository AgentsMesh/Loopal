use super::ContextStore;
use crate::compaction::{compact_messages, sanitize_tool_pairs};
use crate::degradation::{drop_oldest_group, run_sync_degradation};
use crate::ingestion::{cap_assistant_server_blocks, cap_tool_results};
use crate::token_counter::{estimate_message_tokens, estimate_messages_tokens};
use loopal_message::{Message, MessageRole};
use tracing::debug;

impl ContextStore {
    pub fn prepare_for_llm(&self) -> Vec<Message> {
        self.messages().to_vec()
    }

    pub fn apply_summary(&mut self, new_messages: Vec<Message>) -> bool {
        let snapshot = self.messages().to_vec();
        self.replace_messages(new_messages);
        self.sanitize();

        if self
            .budget()
            .needs_emergency(estimate_messages_tokens(self.messages()))
        {
            self.replace_messages(snapshot);
            return false;
        }
        self.enforce_budget();
        true
    }

    pub fn emergency_compact(&mut self, keep_last: usize) {
        let msgs = self.messages_mut();
        compact_messages(msgs, keep_last);
        sanitize_tool_pairs(msgs);
        self.enforce_budget();
    }

    pub fn condense_server_blocks(&mut self) {
        crate::ingestion::condense_all_server_blocks(self.messages_mut());
    }

    pub fn needs_summarization(&self) -> bool {
        self.budget()
            .needs_compaction(estimate_messages_tokens(self.messages()))
    }

    pub fn needs_emergency(&self) -> bool {
        self.budget()
            .needs_emergency(estimate_messages_tokens(self.messages()))
    }

    pub fn token_aware_keep_count(&self) -> usize {
        let half = self.budget().message_budget / 2;
        let mut tokens = 0u32;
        let mut count = 0usize;
        for msg in self.messages().iter().rev() {
            let mt = estimate_message_tokens(msg);
            if tokens + mt > half && count > 0 {
                break;
            }
            tokens += mt;
            count += 1;
        }
        count.max(2)
    }

    pub fn current_tokens(&self) -> u32 {
        estimate_messages_tokens(self.messages())
    }

    pub(super) fn enforce_budget(&mut self) {
        let budget = self.budget().clone();
        run_sync_degradation(self.messages_mut(), &budget);

        let mut iterations = 0;
        let mut dropped_any = false;
        while estimate_messages_tokens(self.messages()) > budget.message_budget * 90 / 100
            && iterations < 10
        {
            if drop_oldest_group(self.messages_mut()) == 0 {
                break;
            }
            dropped_any = true;
            iterations += 1;
        }
        if dropped_any {
            self.sanitize();
        }
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
