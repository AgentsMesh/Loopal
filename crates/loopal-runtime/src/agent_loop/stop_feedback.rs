use loopal_provider_api::MessageOrigin;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{InjectionKind, TurnStep};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Push stop-hook feedback as a new User message. Persisted because
    /// stop-hook feedback is real conversational content (replay must see it).
    pub(super) fn push_stop_feedback(&mut self, feedback: String) {
        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: feedback.clone(),
            }],
            origin: Some(MessageOrigin::StopFeedback),
            ephemeral_in_history: false,
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist stop-feedback message");
        }
        if let Err(e) = self.append_step_record(TurnStep::Injection {
            kind: InjectionKind::StopFeedback,
            text: feedback,
        }) {
            error!(error = %e, "append_step(Injection::StopFeedback) failed");
        }
        // reason: dual-write transitional — see ContextStore::refresh_view doc.
        self.params.store.push_user(msg);
    }
}
