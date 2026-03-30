//! Control command methods on SessionController (mode, model, thinking, clear, compact, rewind).

use loopal_protocol::{AgentMode, ControlCommand};

use crate::controller::SessionController;

impl SessionController {
    pub async fn switch_mode(&self, mode: AgentMode) {
        let target = {
            let mut s = self.lock();
            s.mode = match mode {
                AgentMode::Plan => "plan",
                AgentMode::Act => "act",
            }
            .to_string();
            s.active_view.clone()
        };
        self.backend
            .send_control_to_agent(&target, ControlCommand::ModeSwitch(mode))
            .await;
    }

    pub async fn switch_model(&self, model: String) {
        let target = {
            let mut s = self.lock();
            s.model = model.clone();
            let conv = s.active_conversation_mut();
            crate::conversation_display::push_system_msg(
                conv,
                &format!("Switched model to: {model}"),
            );
            s.active_view.clone()
        };
        self.backend
            .send_control_to_agent(&target, ControlCommand::ModelSwitch(model))
            .await;
    }

    pub async fn switch_thinking(&self, config_json: String) {
        let label = crate::conversation_display::thinking_label_from_json(&config_json);
        let target = {
            let mut s = self.lock();
            s.thinking_config = label.clone();
            let conv = s.active_conversation_mut();
            crate::conversation_display::push_system_msg(
                conv,
                &format!("Switched thinking to: {label}"),
            );
            s.active_view.clone()
        };
        self.backend
            .send_control_to_agent(&target, ControlCommand::ThinkingSwitch(config_json))
            .await;
    }

    pub async fn clear(&self) {
        let target = {
            let mut s = self.lock();
            let conv = s.active_conversation_mut();
            conv.messages.clear();
            conv.streaming_text.clear();
            conv.turn_count = 0;
            conv.input_tokens = 0;
            conv.output_tokens = 0;
            conv.cache_creation_tokens = 0;
            conv.cache_read_tokens = 0;
            conv.retry_banner = None;
            conv.reset_timer();
            s.inbox.clear();
            s.active_view.clone()
        };
        self.backend
            .send_control_to_agent(&target, ControlCommand::Clear)
            .await;
    }

    pub async fn compact(&self) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::Compact)
            .await;
    }

    pub async fn rewind(&self, turn_index: usize) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::Rewind { turn_index })
            .await;
    }
}
