use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use tracing::{error, info, warn};

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

    pub(super) async fn handle_permission_switch(&mut self, s: String) -> Result<()> {
        let Ok(mode) = s.parse::<loopal_tool_api::PermissionMode>() else {
            error!(value = %s, "invalid permission mode");
            return Ok(());
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
        Ok(())
    }

    pub(super) async fn handle_decision_switch(&mut self, s: String) -> Result<()> {
        let Ok(mode) = s.parse::<loopal_decision_api::DecisionMode>() else {
            error!(value = %s, "invalid decision mode");
            return Ok(());
        };
        if mode == loopal_decision_api::DecisionMode::Agent {
            warn!("DecisionMode::Agent is not yet implemented; behaves as Classifier");
        }
        info!(%mode, "switching decision mode");
        self.emit(AgentEventPayload::DecisionModeChanged {
            mode: mode.to_string(),
        })
        .await?;
        self.params.decision_cell.set(mode);
        Ok(())
    }

    pub(super) async fn handle_sandbox_switch(&mut self, s: String) -> Result<()> {
        let Ok(policy) = s.parse::<loopal_config::SandboxPolicy>() else {
            error!(value = %s, "invalid sandbox policy");
            return Ok(());
        };
        info!(%policy, "switching sandbox policy");
        self.emit(AgentEventPayload::SandboxPolicyChanged {
            policy: policy.to_string(),
        })
        .await?;
        self.params.deps.kernel.set_sandbox_policy(policy);
        self.rebuild_backend();
        Ok(())
    }
}
