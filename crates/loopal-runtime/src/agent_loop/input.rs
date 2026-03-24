use crate::agent_input::AgentInput;
use crate::mode::AgentMode;
use loopal_error::Result;
use loopal_message::{ContentBlock, ImageSource, Message, MessageRole};
use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope, MessageSource};
use tracing::{error, info};

use super::WaitResult;
use super::rewind::detect_turn_boundaries;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Wait for user input via the frontend. Returns None if disconnected.
    ///
    /// Control commands (mode switch, clear, compact, rewind, etc.) are
    /// handled inline and the wait resumes — only a real user message
    /// or a disconnect exits this function.
    pub async fn wait_for_input(&mut self) -> Result<Option<WaitResult>> {
        // Discard any stale interrupt signal from the previous turn.
        // Entering idle means the prior turn's interrupt has been fully handled.
        self.interrupt.take();
        self.emit(AgentEventPayload::AwaitingInput).await?;
        loop {
            let input = self.params.frontend.recv_input().await;
            match input {
                Some(AgentInput::Message(env)) => {
                    let mut user_msg = build_user_message(&env);
                    if let Err(e) = self
                        .params
                        .session_manager
                        .save_message(&self.params.session.id, &mut user_msg)
                    {
                        error!(error = %e, "failed to persist message");
                    }
                    self.params.store.push_user(user_msg);
                    return Ok(Some(WaitResult::MessageAdded));
                }
                Some(AgentInput::Control(ctrl)) => {
                    self.handle_control(ctrl).await?;
                }
                None => {
                    info!("input channel closed, ending agent loop");
                    return Ok(None);
                }
            }
        }
    }

    /// Handle a control command; caller resumes waiting for user input.
    async fn handle_control(&mut self, ctrl: ControlCommand) -> Result<()> {
        match ctrl {
            ControlCommand::ModeSwitch(new_mode) => {
                self.params.mode = AgentMode::from(new_mode);
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
                    context_window: self.model_config.max_context_tokens,
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
                info!(from = %self.params.model, to = %new_model, "switching model");
                self.model_config.update_model(&new_model);
                self.params.model = new_model;
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

    async fn handle_rewind(&mut self, turn_index: usize) -> Result<()> {
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
                .session_manager
                .clear_history(&self.params.session.id)
            {
                error!(error = %e, "failed to persist clear marker for rewind");
            }
        } else if let Some(ref id) = self.params.store.messages()[truncate_at].id {
            if let Err(e) = self
                .params
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

/// Build a user Message from an Envelope, converting UserContent into ContentBlocks.
pub fn build_user_message(env: &Envelope) -> Message {
    let text = match &env.source {
        MessageSource::Human => env.content.text.clone(),
        MessageSource::Agent(name) => format!("[from: {}] {}", name, env.content.text),
        MessageSource::Channel { channel, from } => {
            format!("[from: #{}/{}] {}", channel, from, env.content.text)
        }
    };
    let mut blocks: Vec<ContentBlock> = Vec::new();
    if !text.is_empty() {
        blocks.push(ContentBlock::Text { text });
    }
    for img in &env.content.images {
        blocks.push(ContentBlock::Image {
            source: ImageSource {
                source_type: "base64".to_string(),
                media_type: img.media_type.clone(),
                data: img.data.clone(),
            },
        });
    }
    Message {
        id: None,
        role: MessageRole::User,
        content: blocks,
    }
}
