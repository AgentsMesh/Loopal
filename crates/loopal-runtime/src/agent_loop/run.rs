//! Outer loop: user-interaction granularity.
//!
//! Extracted from runner.rs to keep files under 200 lines.

use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_protocol::AgentEventPayload;
use tracing::{error, info};

use super::WaitResult;
use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Outer loop: user-interaction granularity.
    pub(super) async fn run_loop(&mut self) -> Result<AgentOutput> {
        let mut last_output = String::new();
        loop {
            info!(
                turn = self.turn_count,
                messages = self.params.messages.len(),
                "turn start"
            );

            if self.params.messages.is_empty() {
                if !self.params.interactive {
                    break;
                }
                match self.wait_for_input().await? {
                    Some(WaitResult::MessageAdded) => {}
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
            let cancel = TurnCancel::new(self.interrupt.clone(), self.interrupt_notify.clone());
            match self.execute_turn(&cancel).await {
                Ok(turn) => {
                    if !turn.output.is_empty() {
                        last_output.clone_from(&turn.output);
                    }

                    // Atomically check + clear interrupt (avoids losing a second signal)
                    if self.interrupt.take() {
                        self.emit_interrupted().await?;
                        match self.wait_for_input().await? {
                            Some(WaitResult::MessageAdded) => {
                                self.turn_count += 1;
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
                        }
                        None => break,
                    }
                }
                Err(e) => {
                    // Interrupt during LLM call may surface as a network error — handle gracefully
                    if self.interrupt.take() {
                        self.emit_interrupted().await?;
                        match self.wait_for_input().await? {
                            Some(WaitResult::MessageAdded) => {
                                self.turn_count += 1;
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

    /// Emit Interrupted event to TUI. Signal is already consumed by `take()`.
    async fn emit_interrupted(&mut self) -> Result<()> {
        info!("agent interrupted by user");
        self.emit(AgentEventPayload::Interrupted).await
    }
}
