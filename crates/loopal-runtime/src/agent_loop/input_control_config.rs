use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use tracing::{error, info};

use super::input_control::ControlOutcome;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Rebuild `tool_ctx.backend` from the kernel's current sandbox policy +
    /// session cwd/id. Shared by `SandboxPolicySwitch` and session resume.
    pub(super) fn rebuild_backend(&mut self) {
        self.tool_ctx.backend = self.params.deps.kernel.create_backend(
            std::path::Path::new(&self.params.session.cwd),
            &self.params.session.id,
        );
    }

    pub(super) async fn handle_permission_switch(&mut self, s: String) -> Result<ControlOutcome> {
        let Ok(mode) = s.parse::<loopal_tool_api::PermissionMode>() else {
            error!(value = %s, "invalid permission mode");
            return Ok(ControlOutcome::rejected(format!(
                "invalid permission mode: {s}"
            )));
        };
        info!(?mode, "switching permission mode");
        self.emit(AgentEventPayload::PermissionModeChanged {
            mode: mode.to_string(),
        })
        .await?;
        self.params.config.permission_mode = mode;
        // A /permission switch during plan mode must survive plan exit, so
        // update the snapshot too — else restore_pre_plan_state reverts it.
        if let Some(ps) = self.params.config.plan_state.as_mut() {
            ps.previous_permission_mode = mode;
        }
        Ok(ControlOutcome::applied())
    }

    pub(super) async fn handle_thinking_switch(&mut self, json: String) -> Result<ControlOutcome> {
        let config = match serde_json::from_str::<loopal_provider_api::ThinkingConfig>(&json) {
            Ok(config) => config,
            Err(error) => {
                error!(%error, "invalid thinking config");
                return Ok(ControlOutcome::rejected(format!(
                    "invalid thinking config: {error}"
                )));
            }
        };
        info!(thinking = ?config, "switching thinking config");
        // Emit-first: all runtime and auxiliary request state stays unchanged
        // when view-state cannot observe the switch.
        self.emit(AgentEventPayload::ThinkingChanged {
            thinking_config: json,
        })
        .await?;
        self.model_config.thinking = config.clone();
        self.params.config.thinking_config = config.clone();
        if let Some(state) = &self.params.config.thinking_state {
            state.set(config.clone());
        }
        if let Ok(value) = serde_json::to_value(&config) {
            super::input_control::persist_local_setting(
                &self.params.session.cwd,
                "thinking",
                value,
            );
        }
        Ok(ControlOutcome::applied())
    }

    pub(super) async fn handle_decision_switch(&mut self, s: String) -> Result<ControlOutcome> {
        let Ok(mode) = s.parse::<loopal_decision_api::DecisionMode>() else {
            error!(value = %s, "invalid decision mode");
            return Ok(ControlOutcome::rejected(format!(
                "invalid decision mode: {s}"
            )));
        };
        if mode == loopal_decision_api::DecisionMode::Agent {
            return Ok(ControlOutcome::rejected(
                "decision mode 'agent' is not implemented; use 'classifier'",
            ));
        }
        info!(%mode, "switching decision mode");
        self.emit(AgentEventPayload::DecisionModeChanged {
            mode: mode.to_string(),
        })
        .await?;
        self.params.decision_cell.set(mode);
        Ok(ControlOutcome::applied())
    }

    pub(super) async fn handle_sandbox_switch(&mut self, s: String) -> Result<ControlOutcome> {
        let Ok(policy) = s.parse::<loopal_config::SandboxPolicy>() else {
            error!(value = %s, "invalid sandbox policy");
            return Ok(ControlOutcome::rejected(format!(
                "invalid sandbox policy: {s}"
            )));
        };
        info!(%policy, "switching sandbox policy");
        self.emit(AgentEventPayload::SandboxPolicyChanged {
            policy: policy.to_string(),
        })
        .await?;
        self.params.deps.kernel.set_sandbox_policy(policy);
        self.rebuild_backend();
        Ok(ControlOutcome::applied())
    }
}
