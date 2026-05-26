use super::ProjectedView;
use crate::compact_config::IngestionCaps;
use crate::token_counter::estimate_messages_tokens;
use loopal_provider_api::Message;

impl ProjectedView {
    pub fn prepare_for_llm(&self) -> Vec<Message> {
        self.messages().to_vec()
    }

    pub fn needs_summarization(&self) -> bool {
        self.budget().needs_compaction(self.effective_tokens())
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

    pub(super) fn apply_ingestion_caps(&mut self) {
        use crate::ingestion::{cap_assistant_server_blocks, cap_tool_results};
        use loopal_provider_api::MessageRole;
        let caps = IngestionCaps::DEFAULT;
        let max_server = self.budget().message_budget / caps.server_block_divisor;
        let max_result = self.budget().message_budget / caps.tool_result_divisor;
        for msg in self.messages_mut() {
            if msg.role == MessageRole::Assistant {
                cap_assistant_server_blocks(msg, max_server);
            } else if msg.role == MessageRole::User {
                cap_tool_results(msg, max_result);
            }
        }
    }
}
