//! Session display state operations: messages, welcome, history, inbox.

use loopal_protocol::UserContent;

use crate::controller::SessionController;
use crate::helpers::push_system_msg;
use crate::types::DisplayMessage;

impl SessionController {
    pub fn pop_inbox_to_edit(&self) -> Option<UserContent> {
        self.lock().inbox.pop_back()
    }

    pub fn push_system_message(&self, content: String) {
        push_system_msg(&mut self.lock(), &content);
    }

    pub fn push_welcome(&self, model: &str, path: &str) {
        self.lock().messages.push(DisplayMessage {
            role: "welcome".into(),
            content: format!("{model}\n{path}"),
            tool_calls: Vec::new(),
            image_count: 0,
        });
    }

    pub fn load_display_history(&self, display_msgs: Vec<DisplayMessage>) {
        self.lock().messages = display_msgs;
    }
}
