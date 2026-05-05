use loopal_message::{ContentBlock, Message, MessageRole};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Push stop-hook feedback as a new User message. Persisted because
    /// stop-hook feedback is real conversational content (replay must see it).
    pub(super) fn push_stop_feedback(&mut self, feedback: String) {
        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Text { text: feedback }],
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist stop-feedback message");
        }
        self.params.store.push_user(msg);
    }
}
