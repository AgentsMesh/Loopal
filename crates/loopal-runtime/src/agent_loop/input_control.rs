//! Control command handling — mode switch, clear, compact, rewind, etc.

use std::path::PathBuf;

use crate::mode::AgentMode;
use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, ControlCommand};
use tracing::{error, info};

use super::rewind::detect_turn_boundaries;
use super::runner::AgentLoopRunner;

fn persist_local_setting(cwd: &str, key: &str, value: serde_json::Value) {
    let cwd = PathBuf::from(cwd);
    if let Err(e) = loopal_config::update_local_settings_field(&cwd, key, value) {
        error!(error = %e, key, "failed to persist setting to settings.local.json");
    }
}

impl AgentLoopRunner {
    /// Handle a control command. Returns `true` when handling injected a
    /// synthetic User envelope (currently only goal kickoff) — idle-phase
    /// callers must propagate that as `WaitResult::MessageAdded`.
    pub(super) async fn handle_control(&mut self, ctrl: ControlCommand) -> Result<bool> {
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
                let context_window = self.params.store.budget().context_window;
                // Emit before mutating local state: if the receiver is gone
                // the runner stays in a state where the next view/snapshot
                // is still coherent rather than `agent: empty, view: full`.
                self.emit(AgentEventPayload::Cleared { context_window })
                    .await?;
                if let Err(e) = self
                    .params
                    .deps
                    .session_manager
                    .clear_history(&self.params.session.id)
                {
                    // Best-effort persist: a failure here leaves the on-disk
                    // marker missing while view + in-memory store are wiped.
                    // The agent will reload pre-clear history on next restart
                    // — acceptable because the alternative (abort the clear,
                    // leave view confused) is worse UX.
                    error!(error = %e, "failed to persist clear marker");
                }
                self.params.store.clear();
                self.turn_count = 0;
                self.tokens.reset();
                self.cleanup_session_tmp().await;
            }
            ControlCommand::Compact { instructions } => {
                self.force_compact(instructions).await?;
            }
            ControlCommand::ModelSwitch(new_model) => {
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
                self.handle_rewind(turn_index).await?;
            }
            ControlCommand::ThinkingSwitch(json) => {
                match serde_json::from_str::<loopal_provider_api::ThinkingConfig>(&json) {
                    Ok(config) => {
                        info!(thinking = ?config, "switching thinking config");
                        // Emit-first; on failure the runner keeps the old
                        // thinking config and view-state is not misinformed.
                        self.emit(AgentEventPayload::ThinkingChanged {
                            thinking_config: json,
                        })
                        .await?;
                        self.model_config.thinking = config.clone();
                        if let Ok(value) = serde_json::to_value(&config) {
                            persist_local_setting(&self.params.session.cwd, "thinking", value);
                        }
                    }
                    Err(e) => error!(error = %e, "invalid thinking config"),
                }
            }
            ControlCommand::ResumeSession(session_id) => {
                self.handle_resume_session(&session_id).await?;
            }
            ControlCommand::QueryMcpStatus => {
                self.handle_query_mcp_status().await?;
            }
            ControlCommand::McpReconnect { server } => {
                self.handle_mcp_reconnect(server).await?;
            }
            ControlCommand::McpDisconnect { server } => {
                self.handle_mcp_disconnect(server).await?;
            }
            ControlCommand::BgTaskKill { id } => {
                self.handle_bg_task_kill(id).await;
            }
            ControlCommand::CronDelete { id } => {
                self.handle_cron_delete(id).await;
            }
            ctrl @ (ControlCommand::GoalCreate { .. }
            | ControlCommand::GoalUserPause
            | ControlCommand::GoalUserResume
            | ControlCommand::GoalUserComplete
            | ControlCommand::GoalUserReopen
            | ControlCommand::GoalClear) => {
                return self.handle_goal_control(ctrl).await;
            }
        }
        Ok(false)
    }

    pub(super) async fn handle_rewind(&mut self, turn_index: usize) -> Result<()> {
        let boundaries = detect_turn_boundaries(self.params.store.messages());
        if turn_index >= boundaries.len() {
            error!(turn_index, total = boundaries.len(), "invalid turn index");
            return Ok(());
        }
        let truncate_at = boundaries[turn_index];
        info!(turn_index, truncate_at, "rewinding conversation");
        if truncate_at == 0 {
            if let Err(e) = self
                .params
                .deps
                .session_manager
                .clear_history(&self.params.session.id)
            {
                error!(error = %e, "failed to persist clear marker for rewind");
            }
        } else if let Some(ref id) = self.params.store.messages()[truncate_at].id {
            if let Err(e) = self
                .params
                .deps
                .session_manager
                .rewind_to(&self.params.session.id, id)
            {
                error!(error = %e, "failed to persist rewind marker");
            }
        } else {
            error!(
                truncate_at,
                "message at truncate point has no id, skipping marker"
            );
        }
        self.params.store.truncate(truncate_at);
        self.turn_count = self.turn_count.min(turn_index as u32);
        let remaining = detect_turn_boundaries(self.params.store.messages()).len();
        self.emit(AgentEventPayload::Rewound {
            remaining_turns: remaining,
        })
        .await?;
        Ok(())
    }
}
