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
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::ThinkingSwitch(config_json))
            .await;
    }

    pub async fn switch_permission_mode(&self, mode: String) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::PermissionModeSwitch(mode))
            .await;
    }

    pub async fn switch_decision_mode(&self, mode: String) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::DecisionModeSwitch(mode))
            .await;
    }

    pub async fn switch_sandbox_policy(&self, policy: String) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::SandboxPolicySwitch(policy))
            .await;
    }

    pub async fn clear(&self) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::Clear)
            .await;
    }

    pub async fn compact(&self, instructions: Option<String>) {
        let target = self.active_target();
        self.backend
            .send_control_to_agent(&target, ControlCommand::Compact { instructions })
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
