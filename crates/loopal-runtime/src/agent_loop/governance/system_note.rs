use loopal_context::ContextStore;
use loopal_provider_api::MessageOrigin;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{InjectionKind, TurnStep};
use tracing::error;

use super::super::runner::AgentLoopRunner;
use super::bridge::DataPlaneBridge;

pub fn make_governance_feedback(feedback: &str) -> Option<Message> {
    if feedback.is_empty() {
        return None;
    }
    Some(Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: feedback.to_string(),
        }],
        origin: Some(MessageOrigin::GovernanceFeedback),
        ephemeral_in_history: false,
    })
}

impl AgentLoopRunner {
    // reason: fail-closed atomicity — if JSONL persist fails, skip the
    // in-memory push so store and JSONL stay consistent. The orphan
    // tool_use is then visible to both views (next-turn LLM sees the gap
    // AND restart-from-JSONL sees the gap), letting upstream recovery
    // logic handle it uniformly. Half-writes break the closure invariant.
    fn persist_and_push<F>(&mut self, mut msg: Message, push: F)
    where
        F: FnOnce(&mut ContextStore, Message),
    {
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "persist failed; skipping store push to keep views consistent");
            return;
        }
        // Domain mirror: pull text + origin off the message to classify the injection.
        let text = msg.text_content();
        if !text.is_empty() {
            let kind = match msg.origin {
                Some(MessageOrigin::GovernanceFeedback)
                | Some(MessageOrigin::GovernanceCompensation) => Some(InjectionKind::Governance),
                Some(MessageOrigin::StopFeedback) => Some(InjectionKind::StopFeedback),
                Some(MessageOrigin::ConfigRefresh) => Some(InjectionKind::ConfigRefresh),
                Some(MessageOrigin::Other { .. }) => Some(InjectionKind::SystemNote),
                _ => None,
            };
            if let Some(kind) = kind {
                self.append_step_record(TurnStep::Injection { kind, text });
            }
        }
        push(&mut self.params.store, msg);
    }
}

impl DataPlaneBridge for AgentLoopRunner {
    fn write_tool_result_stub(&mut self, msg: Message) {
        self.persist_and_push(msg, ContextStore::push_tool_results);
    }

    fn push_system_note(&mut self, msg: Message) {
        self.persist_and_push(msg, ContextStore::push_user);
    }
}
