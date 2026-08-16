use std::path::PathBuf;

use crate::mode::AgentMode;
use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, AgentStatus, ControlCommand};
use tracing::{error, info};

use super::runner::AgentLoopRunner;

pub(super) enum ControlOutcome {
    Applied { continuation: bool },
    Rejected(String),
}

impl ControlOutcome {
    pub(super) fn applied() -> Self {
        Self::Applied {
            continuation: false,
        }
    }

    pub(super) fn continuation() -> Self {
        Self::Applied { continuation: true }
    }

    pub(super) fn rejected(reason: impl Into<String>) -> Self {
        Self::Rejected(reason.into())
    }
}

pub(super) fn persist_local_setting(cwd: &str, key: &str, value: serde_json::Value) {
    let cwd = PathBuf::from(cwd);
    if let Err(e) = loopal_config::update_local_settings_field(&cwd, key, value) {
        error!(error = %e, key, "failed to persist setting to settings.local.json");
    }
}

impl AgentLoopRunner {
    /// Handle a control command. Returns `true` when handling injected a
    /// synthetic User envelope (currently only goal kickoff) — idle-phase
    /// callers must propagate that as `WaitResult::MessageAdded`.
    pub(super) async fn handle_control(&mut self, ctrl: ControlCommand) -> Result<ControlOutcome> {
        match ctrl {
            ControlCommand::ModeSwitch(new_mode) => {
                // Emit-first: if the event drops, leave the runner's mode
                // untouched so view-state and agent stay aligned.
                self.emit(AgentEventPayload::ModeChanged {
                    mode: new_mode.as_str().to_string(),
                })
                .await?;
                self.params.config.mode = AgentMode::from(new_mode);
            }
            ControlCommand::Clear => {
                info!("clearing conversation history");
                let context_window = self.turns.view().budget().context_window;
                self.emit(AgentEventPayload::Cleared { context_window })
                    .await?;
                self.clear_turns_record();
                self.turn_count = 0;
                self.tokens.reset();
                self.cleanup_session_tmp().await;
            }
            ControlCommand::Compact { instructions } => {
                let _ = self.force_compact(instructions).await?;
            }
            ControlCommand::ModelSwitch(new_model) => {
                if new_model.trim().is_empty() {
                    return Ok(ControlOutcome::rejected("model must not be empty"));
                }
                info!(from = %self.params.config.model(), to = %new_model, "switching model");
                // Emit-first; abort the swap on emit failure so the model
                // surfaced to view-state matches the runner.
                self.emit(AgentEventPayload::ModelChanged {
                    model: new_model.clone(),
                })
                .await?;
                self.model_config.update_model(&new_model);
                self.params.config.router.set_default(new_model.clone());
                self.recalculate_budget();
                persist_local_setting(
                    &self.params.session.cwd,
                    "model",
                    serde_json::Value::String(new_model),
                );
            }
            ControlCommand::Rewind { turn_index } => {
                return self.handle_rewind(turn_index).await;
            }
            ControlCommand::ThinkingSwitch(json) => {
                return self.handle_thinking_switch(json).await;
            }
            ControlCommand::PermissionModeSwitch(s) => {
                return self.handle_permission_switch(s).await;
            }
            ControlCommand::DecisionModeSwitch(s) => {
                return self.handle_decision_switch(s).await;
            }
            ControlCommand::SandboxPolicySwitch(s) => {
                return self.handle_sandbox_switch(s).await;
            }
            ControlCommand::ResumeSession(session_id) => {
                self.handle_resume_session(&session_id).await?;
            }
            ControlCommand::QueryMcpStatus => {
                self.handle_query_mcp_status().await?;
            }
            ControlCommand::McpReconnect { server } => {
                return self.handle_mcp_reconnect(server).await;
            }
            ControlCommand::McpDisconnect { server } => {
                return self.handle_mcp_disconnect(server).await;
            }
            ControlCommand::BgTaskKill { id } => {
                return Ok(self.handle_bg_task_kill(id).await);
            }
            ControlCommand::CronDelete { id } => {
                return Ok(self.handle_cron_delete(id).await);
            }
            ctrl @ (ControlCommand::GoalCreate { .. }
            | ControlCommand::GoalUserPause
            | ControlCommand::GoalUserResume
            | ControlCommand::GoalUserComplete
            | ControlCommand::GoalUserReopen
            | ControlCommand::GoalClear) => {
                return self.handle_goal_control(ctrl).await;
            }
            ControlCommand::Suspend => {
                info!("session suspend requested");
                self.continuation_gate
                    .close(super::continuation_gate::GateClose::UserSuspend);
                self.transition(AgentStatus::Suspended).await?;
                let gate = self.continuation_gate.summary();
                self.emit(AgentEventPayload::ContinuationGateChanged(gate))
                    .await?;
            }
            ControlCommand::Unsuspend => {
                info!("session unsuspend requested");
                self.continuation_gate.open_for_envelope();
                let gate = self.continuation_gate.summary();
                self.emit(AgentEventPayload::ContinuationGateChanged(gate))
                    .await?;
                self.transition(AgentStatus::Running).await?;
            }
        }
        Ok(ControlOutcome::applied())
    }

    pub(super) async fn handle_rewind(&mut self, turn_index: usize) -> Result<ControlOutcome> {
        // reason: TUI picker counts user messages in display projection;
        // TurnStore may contain interleaved synthetic Resume turns (from
        // `/compact while idle`) whose indices don't line up. Map the
        // picker ordinal to a real TurnStore index by indexing UserInput
        // triggers only.
        let real_index = match self.turns.store().user_input_turn_index(turn_index) {
            Some(idx) => idx,
            None => {
                error!(
                    user_ordinal = turn_index,
                    total = self.turns.store().turns().len(),
                    "invalid user-turn ordinal for rewind"
                );
                return Ok(ControlOutcome::rejected(format!(
                    "rewind turn index {turn_index} does not exist"
                )));
            }
        };
        info!(
            user_ordinal = turn_index,
            real_index, "rewinding conversation"
        );
        self.rewind_turns_record(real_index);
        self.turn_count = self.turn_count.min(real_index as u32);
        self.emit(AgentEventPayload::Rewound {
            remaining_turns: real_index,
        })
        .await?;
        Ok(ControlOutcome::applied())
    }
}
