//! Synthetic message injection for auto-continuation compatibility.
//!
//! When thinking mode is active, the Anthropic API rejects "assistant message
//! prefill" — the conversation must end with a user message. The standard
//! auto-continuation flow records an assistant message and loops back, which
//! violates this constraint.
//!
//! This module provides helpers that inject a synthetic user message when
//! needed, preserving the normal prefill behavior for non-thinking providers.

use loopal_message::{ContentBlock, Message, MessageRole};
use tracing::error;

use super::runner::AgentLoopRunner;

/// Synthetic prompt injected when the LLM must continue but thinking mode
/// forbids assistant-message prefill.
const CONTINUE_PROMPT: &str = "[Continue from where you left off]";

impl AgentLoopRunner {
    /// Inject a synthetic user message if the provider forbids prefill with thinking.
    ///
    /// Called before `continue` in auto-continuation paths (MaxTokens,
    /// PauseTurn, stream truncation). When the provider allows prefill, the
    /// model resumes from the partial assistant message directly, so no
    /// injection is needed.
    pub(super) fn push_continuation_if_thinking(&mut self) {
        if !self.needs_continuation_injection() {
            return;
        }
        self.persist_and_push_user(CONTINUE_PROMPT);
    }

    /// Push a new user message with stop-hook feedback.
    ///
    /// After `record_assistant_message`, the last message in the store is
    /// Assistant. The old `append_warnings_to_last_user` would violate its
    /// own `debug_assert!(role == User)`. This method correctly creates a
    /// new User message regardless of thinking mode.
    pub(super) fn push_stop_feedback(&mut self, feedback: String) {
        self.persist_and_push_user(&feedback);
    }

    /// Construct, persist, and push a User message with the given text.
    /// System-injected (auto-continuation, stop feedback) — bypasses the inbox
    /// pipeline so it does not surface as an `InboxEnqueued` event to the UI.
    fn persist_and_push_user(&mut self, text: &str) {
        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist continuation message");
        }
        self.params.store.push_user(msg);
    }
}
