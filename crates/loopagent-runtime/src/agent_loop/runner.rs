use loopagent_provider::get_model_info;
use loopagent_types::command::UserCommand;
use loopagent_types::error::{LoopAgentError, Result};
use loopagent_types::event::AgentEvent;
use loopagent_types::message::Message;
use loopagent_types::tool::ToolContext;
use tracing::{error, info, warn};

use crate::mode::AgentMode;

use super::{compact_messages, AgentLoopParams, WaitResult};

/// Encapsulates the agent loop state and behavior.
/// Splitting the 500-line monolithic function into focused methods
/// improves readability, testability, and maintainability.
pub(crate) struct AgentLoopRunner {
    pub(crate) params: AgentLoopParams,
    pub(crate) tool_ctx: ToolContext,
    pub(crate) turn_count: u32,
    pub(crate) total_input_tokens: u32,
    pub(crate) total_output_tokens: u32,
    pub(crate) max_context_tokens: u32,
    pub(crate) max_output_tokens: u32,
}

impl AgentLoopRunner {
    pub(crate) fn new(params: AgentLoopParams) -> Self {
        let tool_ctx = ToolContext {
            cwd: std::path::PathBuf::from(&params.session.cwd),
            session_id: params.session.id.clone(),
        };

        let model_info = get_model_info(&params.model);
        let max_context_tokens = model_info.as_ref().map_or(200_000, |m| m.context_window);
        let max_output_tokens = model_info.as_ref().map_or(16_384, |m| m.max_output_tokens);

        Self {
            params,
            tool_ctx,
            turn_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            max_context_tokens,
            max_output_tokens,
        }
    }

    /// Main loop — orchestrates input, middleware, LLM, and tool execution.
    pub(crate) async fn run(&mut self) -> Result<()> {
        info!(
            session_id = %self.params.session.id,
            model = %self.params.model,
            mode = ?self.params.mode,
            "agent loop started"
        );

        self.emit(AgentEvent::Started).await?;

        loop {
            info!(
                turn = self.turn_count,
                mode = ?self.params.mode,
                messages = self.params.messages.len(),
                "turn start"
            );

            // If no messages yet, wait for user input first
            if self.params.messages.is_empty() {
                match self.wait_for_input().await? {
                    Some(WaitResult::Continue) => continue,
                    Some(WaitResult::MessageAdded) => {}
                    None => break,
                }
            }

            // Execute middleware pipeline
            if !self.execute_middleware().await? {
                break;
            }

            // Check turn limit
            if self.turn_count >= self.params.max_turns {
                self.emit(AgentEvent::MaxTurnsReached { turns: self.turn_count }).await?;
                break;
            }

            // Stream LLM response
            let (assistant_text, tool_uses, stream_error) = self.stream_llm().await?;

            // If stream errored with no useful data, recover
            if stream_error && tool_uses.is_empty() && assistant_text.is_empty() {
                match self.wait_for_input().await? {
                    Some(WaitResult::MessageAdded) => {
                        self.turn_count += 1;
                        continue;
                    }
                    Some(WaitResult::Continue) => continue,
                    None => break,
                }
            }

            // Record assistant message
            self.record_assistant_message(&assistant_text, &tool_uses);

            // Execute tools or wait for more input
            if !tool_uses.is_empty() {
                self.execute_tools(tool_uses).await?;
                self.turn_count += 1;
                continue;
            }

            // No tool use — assistant finished, wait for user input
            match self.wait_for_input().await? {
                Some(WaitResult::Continue) => continue,
                Some(WaitResult::MessageAdded) => {
                    self.turn_count += 1;
                }
                None => break,
            }
        }

        self.emit(AgentEvent::Finished).await?;
        Ok(())
    }

    /// Send an event to the TUI. Returns Err if the channel is closed.
    pub(crate) async fn emit(&self, event: AgentEvent) -> Result<()> {
        self.params.event_tx.send(event).await.map_err(|e| {
            warn!(error = %e, "event channel closed, TUI disconnected");
            LoopAgentError::Other("event channel closed: TUI disconnected".into())
        })
    }

    /// Wait for user input. Returns None if channel closed (should break loop).
    pub(crate) async fn wait_for_input(&mut self) -> Result<Option<WaitResult>> {
        self.emit(AgentEvent::AwaitingInput).await?;
        match self.params.input_rx.recv().await {
            Some(UserCommand::ModeSwitch(new_mode)) => {
                self.params.mode = AgentMode::from(new_mode);
                let mode_str = match new_mode {
                    loopagent_types::command::AgentMode::Plan => "plan",
                    loopagent_types::command::AgentMode::Act => "act",
                };
                self.emit(AgentEvent::ModeChanged {
                    mode: mode_str.to_string(),
                })
                .await?;
                Ok(Some(WaitResult::Continue))
            }
            Some(UserCommand::Message(user_input)) => {
                let user_msg = Message::user(&user_input);
                if let Err(e) = self.params.session_manager.save_message(&self.params.session.id, &user_msg) {
                    error!(error = %e, "failed to persist message");
                }
                self.params.messages.push(user_msg);
                Ok(Some(WaitResult::MessageAdded))
            }
            Some(UserCommand::Clear) => {
                info!("clearing conversation history");
                self.params.messages.clear();
                self.turn_count = 0;
                self.total_input_tokens = 0;
                self.total_output_tokens = 0;
                self.emit(AgentEvent::TokenUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                    context_window: self.max_context_tokens,
                })
                .await?;
                Ok(Some(WaitResult::Continue))
            }
            Some(UserCommand::Compact) => {
                info!(
                    messages_before = self.params.messages.len(),
                    "compacting messages"
                );
                compact_messages(&mut self.params.messages, 10);
                info!(
                    messages_after = self.params.messages.len(),
                    "compaction complete"
                );
                Ok(Some(WaitResult::Continue))
            }
            Some(UserCommand::ModelSwitch(new_model)) => {
                info!(from = %self.params.model, to = %new_model, "switching model");
                let model_info = get_model_info(&new_model);
                self.max_context_tokens = model_info.as_ref().map_or(200_000, |m| m.context_window);
                self.max_output_tokens = model_info.as_ref().map_or(16_384, |m| m.max_output_tokens);
                self.params.model = new_model;
                Ok(Some(WaitResult::Continue))
            }
            None => {
                info!("input channel closed, ending agent loop");
                Ok(None)
            }
        }
    }
}
