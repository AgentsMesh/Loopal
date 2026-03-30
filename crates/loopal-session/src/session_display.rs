//! Session display state operations: messages, welcome, history, inbox.

use loopal_protocol::UserContent;

use crate::controller::SessionController;
use crate::conversation_display::push_system_msg;
use crate::state::ROOT_AGENT;
use crate::types::DisplayMessage;

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
        conv.messages.push(DisplayMessage {
            role: "welcome".into(),
            content: format!("{model}\n{path}"),
            tool_calls: Vec::new(),
            image_count: 0,
            skill_info: None,
        });
    }

    pub fn load_display_history(&self, display_msgs: Vec<DisplayMessage>) {
        let mut state = self.lock();
        let conv = &mut state
            .agents
            .get_mut(ROOT_AGENT)
            .expect("main agent missing")
            .conversation;
        conv.messages = display_msgs;
    }
}
