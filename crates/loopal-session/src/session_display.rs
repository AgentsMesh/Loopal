//! Session display state operations: messages, welcome, history, inbox.

use loopal_protocol::{ProjectedMessage, UserContent};

use crate::controller::SessionController;
use crate::conversation_display::push_system_msg;
use crate::state::ROOT_AGENT;
use crate::types::{SessionMessage, SessionToolCall, ToolCallStatus};

impl SessionController {
    pub fn pop_inbox_to_edit(&self) -> Option<UserContent> {
        self.lock().inbox.pop_back()
    }

    pub fn push_system_message(&self, content: String) {
        let mut state = self.lock();
        let conv = state.active_conversation_mut();
        push_system_msg(conv, &content);
    }

    pub fn push_welcome(&self, model: &str, path: &str) {
        let mut state = self.lock();
        let conv = &mut state
            .agents
            .get_mut(ROOT_AGENT)
            .expect("main agent missing")
            .conversation;
        conv.messages.push(SessionMessage {
            role: "welcome".into(),
            content: format!("{model}\n{path}"),
            tool_calls: Vec::new(),
            image_count: 0,
            skill_info: None,
        });
    }

    /// Load projected messages from session history into display state.
    pub fn load_display_history(&self, projected: Vec<ProjectedMessage>) {
        let session_msgs = projected.into_iter().map(into_session_message).collect();
        let mut state = self.lock();
        let conv = &mut state
            .agents
            .get_mut(ROOT_AGENT)
            .expect("main agent missing")
            .conversation;
        conv.messages = session_msgs;
    }
}

/// Convert a ProjectedMessage (pure data) into a SessionMessage (with default state).
pub fn into_session_message(p: ProjectedMessage) -> SessionMessage {
    SessionMessage {
        role: p.role,
        content: p.content,
        tool_calls: p
            .tool_calls
            .into_iter()
            .map(|tc| SessionToolCall {
                id: tc.id,
                name: tc.name.clone(),
                status: if tc.is_error {
                    ToolCallStatus::Error
                } else if tc.result.is_some() {
                    ToolCallStatus::Success
                } else {
                    ToolCallStatus::Pending
                },
                summary: tc.summary,
                result: tc.result,
                tool_input: tc.input,
                batch_id: None,
                started_at: None,
                duration_ms: None,
                progress_tail: None,
                metadata: tc.metadata,
            })
            .collect(),
        image_count: p.image_count,
        skill_info: None,
    }
}
