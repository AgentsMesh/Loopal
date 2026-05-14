mod budget_control;

use crate::budget::ContextBudget;
use crate::degradation::run_sync_degradation;
use crate::ingestion::{cap_assistant_server_blocks, cap_tool_results};
use loopal_message::{Message, MessageRole};

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

    pub fn from_messages(messages: Vec<Message>, budget: ContextBudget) -> Self {
        let mut store = Self { messages, budget };
        store.apply_ingestion_caps();
        run_sync_degradation(&mut store.messages, &store.budget);
        store
    }

    pub fn update_budget(&mut self, budget: ContextBudget) {
        self.budget = budget;
        self.enforce_budget();
    }

    pub fn push_user(&mut self, msg: Message) {
        debug_assert!(msg.role == MessageRole::User);
        self.messages.push(msg);
        self.enforce_budget();
    }

    pub fn push_assistant(&mut self, mut msg: Message) {
        debug_assert!(msg.role == MessageRole::Assistant);
        let max_server_tokens = self.budget.message_budget / 4;
        cap_assistant_server_blocks(&mut msg, max_server_tokens);
        self.messages.push(msg);
        self.enforce_budget();
    }

    pub fn push_tool_results(&mut self, mut msg: Message) {
        debug_assert!(msg.role == MessageRole::User);
        let max_per_result = self.budget.message_budget / 8;
        cap_tool_results(&mut msg, max_per_result);
        self.messages.push(msg);
        self.enforce_budget();
    }

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

    pub fn last_role(&self) -> Option<MessageRole> {
        self.messages.last().map(|m| m.role)
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn truncate(&mut self, at: usize) {
        self.messages.truncate(at);
    }

    pub(super) fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    pub(super) fn replace_messages(&mut self, new: Vec<Message>) {
        self.messages = new;
    }
}
