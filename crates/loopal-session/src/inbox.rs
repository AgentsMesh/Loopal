//! Inbox queue for buffering user messages when the agent is busy.
use std::collections::VecDeque;

use loopal_protocol::UserContent;

pub struct Inbox {
    queue: VecDeque<UserContent>,
}

impl Inbox {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, content: UserContent) {
        self.queue.push_back(content);
    }

    pub fn pop_front(&mut self) -> Option<UserContent> {
        self.queue.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<UserContent> {
        self.queue.pop_back()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &UserContent> {
        self.queue.iter()
    }
}

impl Default for Inbox {
    fn default() -> Self {
        Self::new()
    }
}

/// Try forwarding a queued inbox message when the active-view agent is idle.
///
/// The inbox is global (session-level): messages are always routed to the
/// currently active agent, regardless of which agent they were typed for.
pub(crate) fn try_forward_inbox(state: &mut crate::state::SessionState) -> Option<UserContent> {
    let agent = state.agents.get_mut(&state.active_view)?;
    if !agent.conversation.agent_idle {
        tracing::debug!("inbox: agent busy, message queued");
        return None;
    }
    let content = state.inbox.pop_front()?;
    tracing::debug!(text_len = content.text.len(), "inbox: forwarding message");
    let image_count = content.images.len();
    let mut display_text = content.text.clone();
    if image_count > 0 {
        display_text.push_str(&format!(" [+{image_count} image(s)]"));
    }
    let skill_info = content.skill_info.clone();
    agent.conversation.agent_idle = false;
    agent.conversation.begin_turn();
    agent
        .conversation
        .messages
        .push(crate::types::SessionMessage {
            role: "user".to_string(),
            content: display_text,
            tool_calls: Vec::new(),
            image_count,
            skill_info,
        });
    Some(content)
}
