mod budget_control;

use crate::budget::ContextBudget;
use loopal_provider_api::{Message, MessageRole, project_turns_to_messages};
use loopal_turn::Turn;

/// Derived projection of `TurnStore.turns` into message-shape, plus budget
/// metadata. Cached so repeated reads in a single turn don't re-project.
/// Refreshed automatically by `TurnTracker` after every mutator.
pub struct ProjectedView {
    messages: Vec<Message>,
    budget: ContextBudget,
    last_actual_input_tokens: Option<u32>,
}

impl ProjectedView {
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            messages: Vec::new(),
            budget,
            last_actual_input_tokens: None,
        }
    }

    pub fn update_budget(&mut self, budget: ContextBudget) {
        self.budget = budget;
    }

    pub(crate) fn refresh_view(&mut self, turns: &[Turn]) {
        self.messages = project_turns_to_messages(turns);
        self.apply_ingestion_caps();
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

    /// Record the prompt_tokens value returned by the provider for the most
    /// recent LLM call. Used to ground `effective_tokens()` in real numbers
    /// instead of the BPE estimate.
    pub fn record_actual_input_tokens(&mut self, tokens: u32) {
        self.last_actual_input_tokens = Some(tokens);
    }

    pub fn last_actual_input_tokens(&self) -> Option<u32> {
        self.last_actual_input_tokens
    }

    pub(super) fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }
}
