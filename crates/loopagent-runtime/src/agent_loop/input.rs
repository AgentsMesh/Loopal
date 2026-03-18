use loopagent_provider::get_model_info;
use loopagent_types::agent_input::AgentInput;
use loopagent_types::control::ControlCommand;
use loopagent_types::envelope::{Envelope, MessageSource};
use loopagent_types::error::Result;
use loopagent_types::event::AgentEventPayload;
use loopagent_types::message::Message;
use tracing::{error, info};

use crate::mode::AgentMode;

use super::{compact_messages, WaitResult};
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Wait for user input via the frontend. Returns None if disconnected.
    pub async fn wait_for_input(&mut self) -> Result<Option<WaitResult>> {
        self.emit(AgentEventPayload::AwaitingInput).await?;

        let input = self.params.frontend.recv_input().await;

        match input {
            Some(AgentInput::Message(env)) => {
                let text = format_envelope_content(&env);
                let user_msg = Message::user(&text);
                if let Err(e) = self.params.session_manager.save_message(&self.params.session.id, &user_msg) {
                    error!(error = %e, "failed to persist message");
                }
                self.params.messages.push(user_msg);
                Ok(Some(WaitResult::MessageAdded))
            }
            Some(AgentInput::Control(ctrl)) => self.handle_control(ctrl).await,
            None => {
                info!("input channel closed, ending agent loop");
                Ok(None)
            }
        }
    }

    /// Handle a control command and return the appropriate wait result.
    async fn handle_control(&mut self, ctrl: ControlCommand) -> Result<Option<WaitResult>> {
        match ctrl {
            ControlCommand::ModeSwitch(new_mode) => {
                self.params.mode = AgentMode::from(new_mode);
                let mode_str = match new_mode {
                    loopagent_types::command::AgentMode::Plan => "plan",
                    loopagent_types::command::AgentMode::Act => "act",
                };
                self.emit(AgentEventPayload::ModeChanged { mode: mode_str.to_string() }).await?;
                Ok(Some(WaitResult::Continue))
            }
            ControlCommand::Clear => {
                info!("clearing conversation history");
                self.params.messages.clear();
                self.turn_count = 0;
                self.total_input_tokens = 0;
                self.total_output_tokens = 0;
                self.total_cache_creation_tokens = 0;
                self.total_cache_read_tokens = 0;
                self.emit(AgentEventPayload::TokenUsage {
                    input_tokens: 0, output_tokens: 0, context_window: self.max_context_tokens,
                    cache_creation_input_tokens: 0, cache_read_input_tokens: 0,
                }).await?;
                Ok(Some(WaitResult::Continue))
            }
            ControlCommand::Compact => {
                info!(before = self.params.messages.len(), "compacting messages");
                compact_messages(&mut self.params.messages, 10);
                info!(after = self.params.messages.len(), "compaction complete");
                Ok(Some(WaitResult::Continue))
            }
            ControlCommand::ModelSwitch(new_model) => {
                info!(from = %self.params.model, to = %new_model, "switching model");
                let model_info = get_model_info(&new_model);
                self.max_context_tokens = model_info.as_ref().map_or(200_000, |m| m.context_window);
                self.max_output_tokens = model_info.as_ref().map_or(16_384, |m| m.max_output_tokens);
                self.params.model = new_model;
                Ok(Some(WaitResult::Continue))
            }
        }
    }
}

/// Format an envelope's content for the LLM message history.
///
/// - `Human` source: no prefix (LLM naturally treats it as user message).
/// - `Agent` / `Channel` source: prepend `[from: X]` so the LLM can
///   distinguish between different message origins.
pub fn format_envelope_content(env: &Envelope) -> String {
    match &env.source {
        MessageSource::Human => env.content.clone(),
        MessageSource::Agent(name) => format!("[from: {}] {}", name, env.content),
        MessageSource::Channel { channel, from } => {
            format!("[from: #{}/{}] {}", channel, from, env.content)
        }
    }
}
