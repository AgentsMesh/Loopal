use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_protocol::AgentStatus;
use loopal_provider_api::MessageRole;
use loopal_provider_api::{ContinuationIntent, ContinuationReason};
use tracing::{error, info};

pub const CONTEXT_OVERFLOW_BANNER: &str = "Context overflow — compacting and retrying...";

use super::LifecycleMode;
use super::cancel::TurnCancel;
use super::input::{PendingInput, QueuedInput};
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn run_loop(&mut self) -> Result<AgentOutput> {
        let mut last_output = String::new();
        let mut last_error: Option<String> = None;
        let mut terminate_reason = TerminateReason::Goal;
        let mut server_block_retry = false;
        let mut context_overflow_retry = false;
        let mut needs_input = !self.has_resumable_turn();

        loop {
            let mut skip_provider_for_input = false;
            let mut workflow_input_error = None;
            // ── Idle phase ──────────────────────────────────────────
            if needs_input {
                let mut ready_for_turn = false;
                let mut queued_input: Option<Box<QueuedInput>> = None;
                let mut input_closed = false;
                if !matches!(self.params.config.lifecycle, LifecycleMode::Ephemeral) {
                    match self.poll_pending_input().await? {
                        PendingInput::Ready(result) => {
                            ready_for_turn = true;
                            skip_provider_for_input = result.blocks_provider();
                            workflow_input_error = result.into_workflow_failure();
                        }
                        PendingInput::Queued(input) => queued_input = Some(input),
                        PendingInput::Empty => {}
                        PendingInput::Closed => input_closed = true,
                    }
                    if self.params.config.lifecycle.is_persistent()
                        && !input_closed
                        && !ready_for_turn
                        && queued_input.is_none()
                    {
                        ready_for_turn = self.goal_continuation_check().await?;
                    }
                }
                if !ready_for_turn {
                    // Suspend owns the Suspended state and already emitted its
                    // AwaitingInput projection while closing the gate.
                    if !matches!(self.status, AgentStatus::Suspended) {
                        self.transition(AgentStatus::WaitingForInput).await?;
                    }
                    // Preserve the ordinary idle projection before shutdown,
                    // while never letting a closed frontend fall through to a
                    // synthetic goal continuation.
                    if input_closed {
                        break;
                    }

                    let wait_for_workflow = self.params.workflow_lease_tracker.has_outstanding();
                    match self.params.config.lifecycle {
                        LifecycleMode::Ephemeral if !wait_for_workflow => {
                            let pending = self.drain_pending_input().await;
                            self.ephemeral_pending_inputs.extend(pending);
                            let Some(env) = self.ephemeral_pending_inputs.pop_front() else {
                                info!("ephemeral agent idle, exiting");
                                break;
                            };
                            let result = self.ingest_message(&env).await;
                            skip_provider_for_input = result.blocks_provider();
                            workflow_input_error = result.into_workflow_failure();
                        }
                        LifecycleMode::Ephemeral
                        | LifecycleMode::Persistent
                        | LifecycleMode::WorkflowEphemeral => {
                            let next_input = {
                                if let Some(input) = queued_input {
                                    Some(self.consume_queued_input(input).await)
                                } else if wait_for_workflow
                                    || self.params.config.lifecycle.is_persistent()
                                {
                                    self.wait_for_input().await?
                                } else {
                                    info!("workflow-aware ephemeral agent idle, exiting");
                                    break;
                                }
                            };
                            match next_input {
                                Some(result) => {
                                    self.interrupt.take();
                                    skip_provider_for_input = result.blocks_provider();
                                    workflow_input_error = result.into_workflow_failure();
                                }
                                None => break,
                            }
                        }
                    }
                }
            }
            needs_input = true;

            if skip_provider_for_input {
                self.emit_inbox_consumed().await;
                if let Some(error) = workflow_input_error {
                    last_output.clear();
                    last_error = Some(error);
                    terminate_reason = TerminateReason::Error;
                    if self.params.config.lifecycle.is_one_shot() {
                        break;
                    }
                }
                continue;
            }

            // ── Running phase ───────────────────────────────────────
            info!(
                turn = self.turn_count,
                messages = self.turns.view().len(),
                "turn start"
            );
            self.transition(AgentStatus::Running).await?;
            // resume / cold-start with a User-tail history skips the idle
            // phase (needs_input=false), so no ingest opened a turn record.
            if !self.ensure_resume_turn_record().await? {
                break;
            }
            // A real new turn supersedes a prior persistent-session failure or
            // interruption. The attempt below will set a new terminal reason if
            // it fails or is cancelled in turn.
            last_error = None;
            terminate_reason = TerminateReason::Goal;
            self.emit_inbox_consumed().await;

            let cancel = TurnCancel::new(self.interrupt.clone(), self.interrupt_tx.clone());
            let mut turn_ctx = TurnContext::new(self.turn_count, cancel);
            // After try_recover, store may end with Assistant. turn_ctx is
            // fresh per-turn, so re-prime intent for ReadyToCall + finalize.
            if !matches!(self.turns.view().last_role(), Some(MessageRole::User)) {
                turn_ctx.pending_continuation = Some(ContinuationIntent::AutoContinue {
                    reason: ContinuationReason::RecoveryRetry,
                });
            }

            match self.execute_turn(&mut turn_ctx).await {
                Ok(turn) => {
                    if !turn.output.is_empty() {
                        last_output.clone_from(&turn.output);
                    }
                    self.turn_count += 1;
                    if self.interrupt.take() {
                        self.collect_interrupted_turn().await?;
                        terminate_reason = TerminateReason::Aborted;
                    } else if self.turns.current_turn_id().is_some() {
                        // is_some guard: skip_stale_continuation_turn already
                        // rewound the turn, leaving no current turn to end.
                        self.end_turn_record(loopal_turn::TurnOutcome::Complete);
                    }
                }
                Err(e) => {
                    if !turn_ctx.best_effort_output().is_empty() {
                        last_output = turn_ctx.best_effort_output().to_owned();
                    }
                    let class = self.classify_turn_error(&e);
                    let recovered = self
                        .try_recover(class, &mut server_block_retry, &mut context_overflow_retry)
                        .await?;
                    if recovered {
                        needs_input = false;
                        continue;
                    }
                    if self.interrupt.take() {
                        self.collect_interrupted_turn().await?;
                        terminate_reason = TerminateReason::Aborted;
                        continue;
                    }
                    error!(error = %e, "LLM request failed");
                    let msg = LoopalError::to_string(&e);
                    self.transition_error(msg.clone()).await?;
                    last_error = Some(msg);
                    terminate_reason = TerminateReason::Error;
                    // reason: an ephemeral agent has no UI/parent to retry, so an
                    // unrecovered turn error must terminate the loop with the real
                    // error — not fall through to "idle, exiting" which would report
                    // a successful empty result and hide the failure from the caller.
                    if self.params.config.lifecycle.is_one_shot() {
                        break;
                    }
                }
            }
            server_block_retry = false;
            context_overflow_retry = false;
        }

        Ok(self.guarded_output(last_output, last_error, terminate_reason))
    }
}
