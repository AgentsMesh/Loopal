//! Session display state operations: messages, welcome, history.

use loopal_protocol::{AgentStatus, ProjectedMessage};

use crate::controller::SessionController;
use crate::conversation_display::push_system_msg;
use crate::state::ROOT_AGENT;
use crate::types::{SessionMessage, SessionToolCall, ToolCallStatus};

impl SessionController {
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
            inbox: None,
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

    /// Load a sub-agent's display history from pre-projected messages.
    ///
    /// Creates the agent entry if it doesn't exist, sets parent/child
    /// relationships, and marks the agent as finished (historical data).
    pub fn load_sub_agent_history(
        &self,
        name: &str,
        session_id: &str,
        parent: Option<&str>,
        model: Option<&str>,
        projected: Vec<ProjectedMessage>,
    ) {
        let display_msgs: Vec<SessionMessage> =
            projected.into_iter().map(into_session_message).collect();
        let mut state = self.lock();
        let agent = state.agents.entry(name.to_string()).or_default();
        agent.parent = parent.map(|s| s.to_string());
        agent.session_id = Some(session_id.to_string());
        if let Some(m) = model {
            agent.observable.model = m.to_string();
        }
        agent.conversation.messages = display_msgs;
        agent.observable.status = AgentStatus::Finished;
        if let Some(parent_name) = parent
            && let Some(parent_agent) = state.agents.get_mut(parent_name)
        {
            let child_name = name.to_string();
            if !parent_agent.children.contains(&child_name) {
                parent_agent.children.push(child_name);
            }
        }
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
        inbox: None,
    }
}
