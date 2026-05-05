use loopal_protocol::{AgentMode, ControlCommand};

use crate::controller::SessionController;

impl SessionController {
    pub async fn send_control(&self, target: String, cmd: ControlCommand) {
        self.backend.send_control_to_agent(&target, cmd).await;
    }

    pub async fn switch_mode(&self, mode: AgentMode) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::ModeSwitch(mode))
            .await;
    }

    pub async fn switch_model(&self, model: String) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::ModelSwitch(model))
            .await;
    }

    pub async fn switch_thinking(&self, config_json: String) {
        let label = thinking_label_from_json(&config_json);
        let target = {
            let mut s = self.lock();
            s.thinking_config = label;
            s.active_view.clone()
        };
        self.backend
            .send_control_to_agent(&target, ControlCommand::ThinkingSwitch(config_json))
            .await;
    }

    pub async fn clear(&self) {
        let target = self.active_target();
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

    pub async fn resume_session(&self, session_id: &str) {
        let target = {
            let mut s = self.lock();
            s.root_session_id = Some(session_id.to_string());
            s.active_view.clone()
        };
        self.backend
            .send_control_to_agent(
                &target,
                ControlCommand::ResumeSession(session_id.to_string()),
            )
            .await;
    }
}

fn thinking_label_from_json(config_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(config_json)
        .ok()
        .and_then(|v| {
            v.get("type")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "auto".to_string())
}
