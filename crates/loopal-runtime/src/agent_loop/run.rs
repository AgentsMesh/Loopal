use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_protocol::{AgentEventPayload, AgentStatus};
use loopal_provider_api::MessageRole;
use loopal_provider_api::{ContinuationIntent, ContinuationReason};
use tracing::{error, info};

pub const CONTEXT_OVERFLOW_BANNER: &str = "Context overflow — compacting and retrying...";

use super::LifecycleMode;
use super::cancel::TurnCancel;
use super::input::WaitResult;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn run_loop(&mut self) -> Result<AgentOutput> {
        let mut last_output = String::new();
        let mut last_error: Option<String> = None;
        let mut server_block_retry = false;
        let mut context_overflow_retry = false;
        // Need user input whenever the last message isn't User — covers empty
        // store, resume-with-Assistant-tail (crash recovery), or any non-User
        // tail. ReadyToCall's invariant would panic the debug_assert otherwise.
        let mut needs_input = !matches!(self.turns.view().last_role(), Some(MessageRole::User));

        loop {
            // ── Idle phase ──────────────────────────────────────────
            if needs_input {
                let injected_continuation =
                    matches!(self.params.config.lifecycle, LifecycleMode::Persistent)
                        && self.goal_continuation_check().await?;
                if !injected_continuation {
                    self.transition(AgentStatus::WaitingForInput).await?;

                    match self.params.config.lifecycle {
                        LifecycleMode::Ephemeral => {
                            let pending = self.drain_pending_input().await;
                            if pending.is_empty() {
                                info!("ephemeral agent idle, exiting");
                                break;
                            }
                            for env in &pending {
                                self.ingest_message(env).await;
                            }
                        }
                        LifecycleMode::Persistent => match self.wait_for_input().await? {
                            Some(WaitResult::MessageAdded) => {
                                self.interrupt.take();
                            }
                            Some(WaitResult::ContinuationInjected) => {
                                self.interrupt.take();
                            }
                            None => break,
                        },
                    }
                }
            }
            needs_input = true;

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
                    } else if self.turns.current_turn_id().is_some() {
                        // is_some guard: skip_stale_continuation_turn already
                        // rewound the turn, leaving no current turn to end.
                        self.end_turn_record(loopal_turn::TurnOutcome::Complete);
                    }
                }
                Err(e) => {
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
                        continue;
                    }
                    error!(error = %e, "LLM request failed");
                    let msg = LoopalError::to_string(&e);
                    self.transition_error(msg.clone()).await?;
                    last_error = Some(msg);
                    // reason: an ephemeral agent has no UI/parent to retry, so an
                    // unrecovered turn error must terminate the loop with the real
                    // error — not fall through to "idle, exiting" which would report
                    // a successful empty result and hide the failure from the caller.
                    if matches!(self.params.config.lifecycle, LifecycleMode::Ephemeral) {
                        break;
                    }
                }
            }
            server_block_retry = false;
            context_overflow_retry = false;
        }

        let (result, terminate_reason) = match last_error {
            Some(err) if last_output.is_empty() => (err, TerminateReason::Error),
            _ => (last_output, TerminateReason::Goal),
        };
        Ok(AgentOutput {
            result,
            terminate_reason,
        })
    }

    pub(super) fn notify_observers_envelope_received(
        &mut self,
        source: &loopal_protocol::MessageSource,
    ) {
        for g in &mut self.governance {
            g.on_envelope_received(source);
        }
    }

    async fn emit_interrupted(&mut self) -> Result<()> {
        info!("agent interrupted by user");
        self.status = AgentStatus::WaitingForInput;
        self.emit(AgentEventPayload::Interrupted).await
    }

    // finalize runs even if the Interrupted emit fails, so the turn is never
    // left InProgress.
    async fn collect_interrupted_turn(&mut self) -> Result<()> {
        let emit_result = self.emit_interrupted().await;
        self.finalize_turn_cancellation(loopal_turn::CancelledCause::UserInterrupt)
            .await;
        emit_result
    }

    pub async fn emit_inbox_consumed(&mut self) {
        let ids = std::mem::take(&mut self.pending_consumed_ids);
        for message_id in ids {
            if let Err(e) = self
                .emit(AgentEventPayload::InboxConsumed {
                    envelope_id: message_id.clone(),
                })
                .await
            {
                tracing::error!(
                    error = %e,
                    message_id = %message_id,
                    "agent_loop::run::emit_inbox_consumed emit failed"
                );
            }
        }
    }
}
