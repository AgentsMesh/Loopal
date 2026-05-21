use loopal_context::ContextStore;
use loopal_message::{ContentBlock, Message, MessageOrigin, MessageRole};
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
