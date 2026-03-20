use loopal_provider::get_model_info;
use crate::agent_input::AgentInput;
use loopal_protocol::ControlCommand;
use loopal_protocol::{Envelope, MessageSource};
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_message::Message;
use tracing::{error, info};

use crate::mode::AgentMode;

use super::rewind::detect_turn_boundaries;
use super::{compact_messages, WaitResult};
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Wait for user input via the frontend. Returns None if disconnected.
    ///
    /// Control commands (mode switch, clear, compact, rewind, etc.) are
    /// handled inline and the wait resumes — only a real user message
    /// or a disconnect exits this function.
    pub async fn wait_for_input(&mut self) -> Result<Option<WaitResult>> {
        self.emit(AgentEventPayload::AwaitingInput).await?;
        loop {
            let input = self.params.frontend.recv_input().await;
            match input {
                Some(AgentInput::Message(env)) => {
                    let text = format_envelope_content(&env);
                    let mut user_msg = Message::user(&text);
                    if let Err(e) = self.params.session_manager.save_message(
                        &self.params.session.id, &mut user_msg,
                    ) {
                        error!(error = %e, "failed to persist message");
                    }
                    self.params.messages.push(user_msg);
                    return Ok(Some(WaitResult::MessageAdded));
                }
                Some(AgentInput::Control(ctrl)) => {
                    self.handle_control(ctrl).await?;
                    // Control command processed — keep waiting for user input.
                }
                None => {
                    info!("input channel closed, ending agent loop");
                    return Ok(None);
                }
            }
        }
    }

    /// Handle a control command. Returns `Ok(())` when done.
    /// The caller (`wait_for_input`) continues waiting for user input.
    async fn handle_control(&mut self, ctrl: ControlCommand) -> Result<()> {
        match ctrl {
            ControlCommand::ModeSwitch(new_mode) => {
                self.params.mode = AgentMode::from(new_mode);
                let mode_str = match new_mode {
                    loopal_protocol::AgentMode::Plan => "plan",
                    loopal_protocol::AgentMode::Act => "act",
                };
                self.emit(AgentEventPayload::ModeChanged { mode: mode_str.to_string() }).await?;
            }
            ControlCommand::Clear => {
                info!("clearing conversation history");
                if let Err(e) = self.params.session_manager.clear_history(&self.params.session.id) {
                    error!(error = %e, "failed to persist clear marker");
                }
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
            }
            ControlCommand::Compact => {
                info!(before = self.params.messages.len(), "compacting messages");
                if let Err(e) = self.params.session_manager.compact_history(&self.params.session.id, 10) {
                    error!(error = %e, "failed to persist compact marker");
                }
                compact_messages(&mut self.params.messages, 10);
                info!(after = self.params.messages.len(), "compaction complete");
            }
            ControlCommand::ModelSwitch(new_model) => {
                info!(from = %self.params.model, to = %new_model, "switching model");
                let model_info = get_model_info(&new_model);
                self.max_context_tokens = model_info.as_ref().map_or(200_000, |m| m.context_window);
                self.max_output_tokens = model_info.as_ref().map_or(16_384, |m| m.max_output_tokens);
                self.params.model = new_model;
            }
            ControlCommand::Rewind { turn_index } => {
                self.handle_rewind(turn_index).await?;
            }
        }
        Ok(())
    }

    async fn handle_rewind(&mut self, turn_index: usize) -> Result<()> {
        let boundaries = detect_turn_boundaries(&self.params.messages);
        if turn_index >= boundaries.len() {
            error!(turn_index, total = boundaries.len(), "invalid turn index");
            return Ok(());
        }
        let truncate_at = boundaries[turn_index];
        info!(turn_index, truncate_at, "rewinding conversation");

        if truncate_at == 0 {
            if let Err(e) = self.params.session_manager.clear_history(&self.params.session.id) {
                error!(error = %e, "failed to persist clear marker for rewind");
            }
        } else if let Some(ref id) = self.params.messages[truncate_at].id {
            if let Err(e) = self.params.session_manager.rewind_to(
                &self.params.session.id, id,
            ) {
                error!(error = %e, "failed to persist rewind marker");
            }
        } else {
            // Message has no id — skip marker; crash-resume restores full history.
            error!(truncate_at, "message at truncate point has no id, skipping marker");
        }

        self.params.messages.truncate(truncate_at);
        self.turn_count = self.turn_count.min(turn_index as u32);
        let remaining = detect_turn_boundaries(&self.params.messages).len();
        self.emit(AgentEventPayload::Rewound { remaining_turns: remaining }).await?;
        Ok(())
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
