//! ContextStore — budget-constrained message buffer.

use crate::budget::ContextBudget;
use crate::compaction::{compact_messages, sanitize_tool_pairs};
use crate::degradation::{drop_oldest_group, run_sync_degradation};
use crate::ingestion::{cap_assistant_server_blocks, cap_tool_results};
use crate::token_counter::{estimate_message_tokens, estimate_messages_tokens};
use loopal_message::{Message, MessageRole};
use tracing::debug;

/// Managed message buffer with budget invariant enforcement.
pub struct ContextStore {
    messages: Vec<Message>,
    budget: ContextBudget,
}

impl ContextStore {
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            messages: Vec::new(),
            budget,
        }
    }

    /// Restore from session replay with normalization.
    pub fn from_messages(messages: Vec<Message>, budget: ContextBudget) -> Self {
        let mut store = Self { messages, budget };
        store.apply_ingestion_caps();
        run_sync_degradation(&mut store.messages, &store.budget);
        store
    }

    /// Update the budget (e.g., after model switch). Re-enforces invariant.
    pub fn update_budget(&mut self, budget: ContextBudget) {
        self.budget = budget;
        self.enforce_budget();
    }

    /// Push a user message.
    pub fn push_user(&mut self, msg: Message) {
        debug_assert!(msg.role == MessageRole::User);
        self.messages.push(msg);
        self.enforce_budget();
    }

    /// Push an assistant message. Caps server blocks exceeding budget/4.
    pub fn push_assistant(&mut self, mut msg: Message) {
        debug_assert!(msg.role == MessageRole::Assistant);
        let max_server_tokens = self.budget.message_budget / 4;
        cap_assistant_server_blocks(&mut msg, max_server_tokens);
        self.messages.push(msg);
        self.enforce_budget();
    }

    /// Push tool-results message. Caps each at budget/8 tokens.
    pub fn push_tool_results(&mut self, mut msg: Message) {
        debug_assert!(msg.role == MessageRole::User);
        let max_per_result = self.budget.message_budget / 8;
        cap_tool_results(&mut msg, max_per_result);
        self.messages.push(msg);
        self.enforce_budget();
    }

    /// Append warning Text blocks to the last User message (after ToolResult blocks).
    pub fn append_warnings_to_last_user(&mut self, warnings: Vec<String>) {
        if warnings.is_empty() {
            return;
        }
        if let Some(msg) = self.messages.last_mut() {
            debug_assert!(msg.role == MessageRole::User);
            for w in warnings {
                msg.content
                    .push(loopal_message::ContentBlock::Text { text: w });
            }
        }
    }

    // --- Read access ---

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn budget(&self) -> &ContextBudget {
        &self.budget
    }

    // --- Lifecycle ---

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn truncate(&mut self, at: usize) {
        self.messages.truncate(at);
    }

    // --- LLM preparation ---

    /// Clone messages for LLM call with final sanitization.
    pub fn prepare_for_llm(&self) -> Vec<Message> {
        let mut msgs = self.messages.clone();
        sanitize_tool_pairs(&mut msgs);
        msgs
    }

    // --- Compaction operations (encapsulated mutation) ---

    /// Apply LLM summarization. Returns false if reverted (tokens inflated).
    pub fn apply_summary(&mut self, new_messages: Vec<Message>) -> bool {
        let snapshot = self.messages.clone();
        self.messages = new_messages;
        sanitize_tool_pairs(&mut self.messages);

        if self
            .budget
            .needs_emergency(estimate_messages_tokens(&self.messages))
        {
            self.messages = snapshot;
            return false;
        }
        self.enforce_budget();
        true
    }

    /// Emergency compaction: drop oldest, keep last N.
    pub fn emergency_compact(&mut self, keep_last: usize) {
        compact_messages(&mut self.messages, keep_last);
        sanitize_tool_pairs(&mut self.messages);
        self.enforce_budget();
    }

    /// Defensive: condense all server blocks when API rejects them.
    pub fn condense_server_blocks(&mut self) {
        crate::ingestion::condense_all_server_blocks(&mut self.messages);
    }

    // --- Query methods for compaction decisions ---

    /// Whether LLM summarization should be attempted (>75% of budget).
    pub fn needs_summarization(&self) -> bool {
        self.budget
            .needs_compaction(estimate_messages_tokens(&self.messages))
    }

    /// Whether emergency degradation is needed (>95% of budget).
    pub fn needs_emergency(&self) -> bool {
        self.budget
            .needs_emergency(estimate_messages_tokens(&self.messages))
    }

    /// How many recent messages fit within 50% of budget.
    pub fn token_aware_keep_count(&self) -> usize {
        let half = self.budget.message_budget / 2;
        let mut tokens = 0u32;
        let mut count = 0usize;
        for msg in self.messages.iter().rev() {
            let mt = estimate_message_tokens(msg);
            if tokens + mt > half && count > 0 {
                break;
            }
            tokens += mt;
            count += 1;
        }
        count.max(2)
    }

    /// Current total message token count.
    pub fn current_tokens(&self) -> u32 {
        estimate_messages_tokens(&self.messages)
    }

    // --- Internal ---

    fn enforce_budget(&mut self) {
        run_sync_degradation(&mut self.messages, &self.budget);

        let mut iterations = 0;
        let mut dropped_any = false;
        while estimate_messages_tokens(&self.messages) > self.budget.message_budget * 90 / 100
            && iterations < 10
        {
            if drop_oldest_group(&mut self.messages) == 0 {
                break;
            }
            dropped_any = true;
            iterations += 1;
        }
        if dropped_any {
            sanitize_tool_pairs(&mut self.messages);
        }
        debug!(
            tokens = estimate_messages_tokens(&self.messages),
            budget = self.budget.message_budget,
            messages = self.messages.len(),
            "budget enforced"
        );
    }

    /// Apply ingestion caps to all messages (used on session reload).
    fn apply_ingestion_caps(&mut self) {
        let max_server = self.budget.message_budget / 4;
        let max_result = self.budget.message_budget / 8;
        for msg in &mut self.messages {
            if msg.role == MessageRole::Assistant {
                cap_assistant_server_blocks(msg, max_server);
            } else if msg.role == MessageRole::User {
                cap_tool_results(msg, max_result);
            }
        }
    }
}
