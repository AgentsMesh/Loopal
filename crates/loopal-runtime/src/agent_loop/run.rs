//! Outer loop: user-interaction granularity.
//!
//! The agent loop runs turns until:
//! - Ephemeral agent: idle with no pending input → exit
//! - Persistent agent: `wait_for_input` returns None (channel closed)
//! - Unrecoverable error
//!
//! State machine: Starting → Running → WaitingForInput → Running → ... → Finished

use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_protocol::{AgentEventPayload, AgentStatus};
use tracing::{error, info};

use super::LifecycleMode;
use super::cancel::TurnCancel;
use super::input::WaitResult;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn run_loop(&mut self) -> Result<AgentOutput> {
        let mut last_output = String::new();
        let mut server_block_retry = false;
        let mut context_overflow_retry = false;
        let mut needs_input = self.params.store.is_empty();

        loop {
            // ── Idle phase ──────────────────────────────────────────
            if needs_input {
                self.transition(AgentStatus::WaitingForInput).await?;

                match self.params.config.lifecycle {
                    LifecycleMode::Ephemeral => {
                        // Messages are delivered directly to the agent mailbox.
                        // drain is reliable — no yield/timeout needed.
                        let pending = self.drain_pending_input().await;
                        if pending.is_empty() {
                            info!("ephemeral agent idle, exiting");
                            break;
                        }
                        for env in &pending {
                            self.ingest_message(env);
                        }
                    }
                    LifecycleMode::Persistent => {
                        // Persistent: block until input arrives or channel closes.
                        match self.wait_for_input().await? {
                            Some(WaitResult::MessageAdded) => {
                                self.interrupt.take();
                                self.notify_observers_user_input();
                            }
                            None => break,
                        }
                    }
                }
            }
            needs_input = true;

            // ── Running phase ───────────────────────────────────────
            info!(
                turn = self.turn_count,
                messages = self.params.store.len(),
                "turn start"
            );
            self.transition(AgentStatus::Running).await?;

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
                }
                Err(e) => {
                    if !server_block_retry && is_server_block_error(&e) {
                        server_block_retry = true;
                        info!("condensing server blocks after API rejection, retrying");
                        self.params.store.condense_server_blocks();
                        needs_input = false;
                        continue;
                    }
                    if !context_overflow_retry && e.is_context_overflow() {
                        context_overflow_retry = true;
                        info!("context overflow detected, emergency compacting and retrying");
                        self.params.store.emergency_compact(5);
                        self.emit(AgentEventPayload::Error {
                            message: "Context overflow — compacting and retrying...".into(),
                        })
                        .await?;
                        needs_input = false;
                        continue;
                    }
                    if self.interrupt.take() {
                        self.emit_interrupted().await?;
                        continue;
                    }
                    error!(error = %e, "LLM request failed");
                    self.transition_error(LoopalError::to_string(&e)).await?;
                }
            }
            server_block_retry = false;
            context_overflow_retry = false;
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
        self.status = AgentStatus::WaitingForInput;
        self.emit(AgentEventPayload::Interrupted).await
    }
}

fn is_server_block_error(e: &LoopalError) -> bool {
    let msg = e.to_string();
    msg.contains("code_execution")
        && (msg.contains("without a corresponding") || msg.contains("tool_result"))
}
