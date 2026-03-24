//! Outer loop: user-interaction granularity.
//!
//! Extracted from runner.rs to keep files under 200 lines.

use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_protocol::AgentEventPayload;
use tracing::{error, info};

use super::WaitResult;
use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    /// Outer loop: user-interaction granularity.
    pub(super) async fn run_loop(&mut self) -> Result<AgentOutput> {
        let mut last_output = String::new();
        loop {
            info!(
                turn = self.turn_count,
                messages = self.params.store.len(),
                "turn start"
            );

            if self.params.store.is_empty() {
                if !self.params.interactive {
                    break;
                }
                match self.wait_for_input().await? {
                    Some(WaitResult::MessageAdded) => {
                        self.notify_observers_user_input();
                    }
                    None => break,
                }
            }

            if self.turn_count >= self.params.max_turns {
                self.emit(AgentEventPayload::MaxTurnsReached {
                    turns: self.turn_count,
                })
                .await?;
                return Ok(AgentOutput {
                    result: last_output,
                    terminate_reason: TerminateReason::MaxTurns,
                });
            }

            // Execute one complete turn (LLM → [tools → LLM]* → done)
            let cancel = TurnCancel::new(self.interrupt.clone(), self.interrupt_tx.clone());
            let mut turn_ctx = TurnContext::new(self.turn_count, cancel);
            match self.execute_turn(&mut turn_ctx).await {
                Ok(turn) => {
                    if !turn.output.is_empty() {
                        last_output.clone_from(&turn.output);
                    }

                    if self.interrupt.take() {
                        self.emit_interrupted().await?;
                        match self.wait_for_input().await? {
                            Some(WaitResult::MessageAdded) => {
                                self.turn_count += 1;
                                self.notify_observers_user_input();
                                continue;
                            }
                            None => break,
                        }
                    }

                    if !self.params.interactive {
                        break;
                    }
                    if self.turn_count >= self.params.max_turns {
                        self.emit(AgentEventPayload::MaxTurnsReached {
                            turns: self.turn_count,
                        })
                        .await?;
                        return Ok(AgentOutput {
                            result: last_output,
                            terminate_reason: TerminateReason::MaxTurns,
                        });
                    }
                    match self.wait_for_input().await? {
                        Some(WaitResult::MessageAdded) => {
                            self.turn_count += 1;
                            self.notify_observers_user_input();
                        }
                        None => break,
                    }
                }
                Err(e) => {
                    if self.interrupt.take() {
                        self.emit_interrupted().await?;
                        match self.wait_for_input().await? {
                            Some(WaitResult::MessageAdded) => {
                                self.turn_count += 1;
                                self.notify_observers_user_input();
                                continue;
                            }
                            None => break,
                        }
                    }
                    if !self.params.interactive {
                        return Ok(AgentOutput {
                            result: last_output,
                            terminate_reason: TerminateReason::Error,
                        });
                    }
                    error!(error = %e, "LLM request failed");
                    self.emit(AgentEventPayload::Error {
                        message: LoopalError::to_string(&e),
                    })
                    .await?;
                    match self.wait_for_input().await? {
                        Some(WaitResult::MessageAdded) => {
                            self.turn_count += 1;
                            self.notify_observers_user_input();
                            continue;
                        }
                        None => break,
                    }
                }
            }
        }

        Ok(AgentOutput {
            result: if self.params.interactive {
                String::new()
            } else {
                last_output
            },
            terminate_reason: TerminateReason::Goal,
        })
    }

    /// Notify all observers that the user sent new input.
    fn notify_observers_user_input(&mut self) {
        for obs in &mut self.observers {
            obs.on_user_input();
        }
    }

    /// Emit Interrupted event to TUI. Signal is already consumed by `take()`.
    async fn emit_interrupted(&mut self) -> Result<()> {
        info!("agent interrupted by user");
        self.emit(AgentEventPayload::Interrupted).await
    }
}
