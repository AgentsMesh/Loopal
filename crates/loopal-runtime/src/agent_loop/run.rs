//! Outer loop: user-interaction granularity.
//!
//! The agent loop runs turns until:
//! - `wait_for_input` returns None (frontend disconnected / channel closed)
//! - The agent encounters an unrecoverable error
//!
//! State machine: Starting → Running → WaitingForInput → Running → ... → Finished

use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_protocol::{AgentEventPayload, AgentStatus};
use tracing::{error, info};

use super::WaitResult;
use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    /// Outer loop: state-machine driven.
    ///
    /// Every agent (interactive or task-oriented) runs the same loop:
    /// 1. Wait for input (idle phase — emit AwaitingInput)
    /// 2. Execute turn (running phase)
    /// 3. Go back to 1
    ///
    /// Agent exits when `wait_for_input()` returns None (channel closed).
    /// Task agents exit naturally because their input channel is closed
    /// after the prompt is delivered (no more messages will arrive).
    pub(super) async fn run_loop(&mut self) -> Result<AgentOutput> {
        let mut last_output = String::new();
        let mut server_block_retry = false;
        // Whether we need to wait for new input before the next turn.
        // False initially when store already has messages (prompt pre-loaded).
        let mut needs_input = self.params.store.is_empty();

        loop {
            // ── Idle phase ──────────────────────────────────────────
            if needs_input {
                self.transition(AgentStatus::WaitingForInput).await?;
                match self.wait_for_input().await? {
                    Some(WaitResult::MessageAdded) => {
                        self.interrupt.take();
                        self.notify_observers_user_input();
                    }
                    None => break, // Channel closed → exit
                }
            }
            // After this point, always wait for input on next iteration.
            needs_input = true;

            // ── Running phase ───────────────────────────────────────
            info!(
                turn = self.turn_count,
                messages = self.params.store.len(),
                "turn start"
            );
            self.status = AgentStatus::Running;

            let cancel = TurnCancel::new(self.interrupt.clone(), self.interrupt_tx.clone());
            let mut turn_ctx = TurnContext::new(self.turn_count, cancel);

            match self.execute_turn(&mut turn_ctx).await {
                Ok(turn) => {
                    if !turn.output.is_empty() {
                        last_output.clone_from(&turn.output);
                    }
                    self.turn_count += 1;

                    if self.interrupt.take() {
                        self.emit_interrupted().await?;
                    }
                    // Turn complete → loop back to idle phase.
                }
                Err(e) => {
                    if !server_block_retry && is_server_block_error(&e) {
                        server_block_retry = true;
                        info!("condensing server blocks after API rejection, retrying");
                        self.params.store.condense_server_blocks();
                        needs_input = false; // Retry without waiting
                        continue;
                    }

                    if self.interrupt.take() {
                        self.emit_interrupted().await?;
                        continue;
                    }

                    error!(error = %e, "LLM request failed");
                    self.emit(AgentEventPayload::Error {
                        message: LoopalError::to_string(&e),
                    })
                    .await?;
                    // Error → back to idle, wait for recovery input or disconnect.
                }
            }
            server_block_retry = false;
        }

        Ok(AgentOutput {
            result: last_output,
            terminate_reason: TerminateReason::Goal,
        })
    }

    fn notify_observers_user_input(&mut self) {
        for obs in &mut self.observers {
            obs.on_user_input();
        }
    }

    async fn emit_interrupted(&mut self) -> Result<()> {
        info!("agent interrupted by user");
        self.emit(AgentEventPayload::Interrupted).await
    }
}

fn is_server_block_error(e: &LoopalError) -> bool {
    let msg = e.to_string();
    msg.contains("code_execution")
        && (msg.contains("without a corresponding") || msg.contains("tool_result"))
}
