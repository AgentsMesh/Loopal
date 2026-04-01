//! Control command handling — mode switch, clear, compact, rewind, etc.

use crate::mode::AgentMode;
use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, ControlCommand};
use tracing::{error, info};

use super::rewind::detect_turn_boundaries;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Handle a control command; caller resumes waiting for user input.
    pub(super) async fn handle_control(&mut self, ctrl: ControlCommand) -> Result<()> {
        match ctrl {
            ControlCommand::ModeSwitch(new_mode) => {
                self.params.config.mode = AgentMode::from(new_mode);
                let mode_str = match new_mode {
                    loopal_protocol::AgentMode::Plan => "plan",
                    loopal_protocol::AgentMode::Act => "act",
                };
                self.emit(AgentEventPayload::ModeChanged {
                    mode: mode_str.to_string(),
                })
                .await?;
            }
            ControlCommand::Clear => {
                info!("clearing conversation history");
                if let Err(e) = self
                    .params
                    .deps
                    .session_manager
                    .clear_history(&self.params.session.id)
                {
                    error!(error = %e, "failed to persist clear marker");
                }
                self.params.store.clear();
                self.turn_count = 0;
                self.tokens.reset();
                self.emit(AgentEventPayload::TokenUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                    context_window: self.params.store.budget().context_window,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                    thinking_tokens: 0,
                })
                .await?;
            }
            ControlCommand::Compact => {
                self.force_compact().await?;
            }
            ControlCommand::ModelSwitch(new_model) => {
                info!(from = %self.params.config.model(), to = %new_model, "switching model");
                self.model_config.update_model(&new_model);
                self.params.config.router.set_default(new_model);
                self.recalculate_budget();
            }
            ControlCommand::Rewind { turn_index } => {
                self.handle_rewind(turn_index).await?;
            }
            ControlCommand::ThinkingSwitch(json) => {
                match serde_json::from_str::<loopal_provider_api::ThinkingConfig>(&json) {
                    Ok(config) => {
                        info!(thinking = ?config, "switching thinking config");
                        self.model_config.thinking = config;
                    }
                    Err(e) => error!(error = %e, "invalid thinking config"),
                }
            }
        }
        Ok(())
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
