mod budget_control;

use std::time::SystemTime;

use crate::budget::ContextBudget;
use crate::degradation::run_sync_degradation;
use crate::ingestion::{cap_assistant_server_blocks, cap_tool_results};
use loopal_provider_api::{Message, MessageRole, project_turns_to_messages};
use loopal_turn::Turn;

pub struct ContextStore {
    messages: Vec<Message>,
    budget: ContextBudget,
    last_actual_input_tokens: Option<u32>,
    last_assistant_activity_at: Option<SystemTime>,
}

impl ContextStore {
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            messages: Vec::new(),
            budget,
            last_actual_input_tokens: None,
            last_assistant_activity_at: None,
        }
    }

    pub fn from_messages(messages: Vec<Message>, budget: ContextBudget) -> Self {
        let mut store = Self {
            messages,
            budget,
            last_actual_input_tokens: None,
            last_assistant_activity_at: None,
        };
        store.apply_ingestion_caps();
        run_sync_degradation(&mut store.messages, &store.budget);
        store
    }

    pub fn update_budget(&mut self, budget: ContextBudget) {
        self.budget = budget;
        self.enforce_budget();
    }

    pub fn refresh_view(&mut self, turns: &[Turn]) {
        self.messages = project_turns_to_messages(turns);
        self.apply_ingestion_caps();
        self.enforce_budget();
        if let Some(at) = latest_llm_call_started_at(turns) {
            self.last_assistant_activity_at = Some(datetime_to_system_time(at));
        }
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
        self.last_assistant_activity_at = Some(SystemTime::now());
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
                    .push(loopal_provider_api::ContentBlock::Text { text: w });
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

    /// Apply microcompaction in place. The store hands the message list to
    /// the middleware so the scrub logic stays out of the runtime; the
    /// runtime still owns the clock + idle threshold and passes them in.
    pub fn apply_microcompact(
        &mut self,
        last_activity: Option<std::time::SystemTime>,
        now: std::time::SystemTime,
        idle_threshold: std::time::Duration,
    ) -> Option<crate::middleware::microcompact::MicroCompactStats> {
        crate::middleware::microcompact::maybe_microcompact(
            &mut self.messages,
            last_activity,
            now,
            idle_threshold,
        )
    }

    /// Record the prompt_tokens value returned by the provider for the most
    /// recent LLM call. Used to ground `effective_tokens()` in real numbers
    /// instead of the BPE estimate, which can drift up to ~30% on Anthropic
    /// payloads (cl100k_base ≠ Anthropic tokenizer).
    pub fn record_actual_input_tokens(&mut self, tokens: u32) {
        self.last_actual_input_tokens = Some(tokens);
    }

    pub fn last_actual_input_tokens(&self) -> Option<u32> {
        self.last_actual_input_tokens
    }

    /// Refresh the "last assistant activity" timestamp. Microcompact uses this
    /// to detect long-idle conversations whose old tool results no longer
    /// share a server-side cache and can be safely scrubbed.
    pub fn record_assistant_activity(&mut self, at: SystemTime) {
        self.last_assistant_activity_at = Some(at);
    }

    pub fn last_assistant_activity_at(&self) -> Option<SystemTime> {
        self.last_assistant_activity_at
    }

    pub(super) fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    pub(super) fn replace_messages(&mut self, new: Vec<Message>) {
        self.messages = new;
    }
}

fn latest_llm_call_started_at(turns: &[Turn]) -> Option<chrono::DateTime<chrono::Utc>> {
    turns
        .iter()
        .rev()
        .find(|t| {
            t.body
                .steps
                .iter()
                .any(|s| matches!(s, loopal_turn::TurnStep::LlmCall { .. }))
        })
        .map(|t| t.started_at)
}

fn datetime_to_system_time(at: chrono::DateTime<chrono::Utc>) -> SystemTime {
    let secs = at.timestamp().max(0) as u64;
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)
}
