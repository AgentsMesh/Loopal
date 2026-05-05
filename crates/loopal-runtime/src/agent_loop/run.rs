use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_message::MessageRole;
use loopal_protocol::{AgentEventPayload, AgentStatus};
use loopal_provider_api::{
    ContinuationIntent, ContinuationReason, ErrorClass, default_classify_error,
};
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
        // Need user input whenever the last message is not a User message.
        // Covers: empty store, resume-with-trailing-Assistant (crash recovery),
        // and any unexpected non-User tail. ReadyToCall's invariant assumes a
        // User tail (or pending continuation), so going straight to the running
        // phase without a User tail would panic the debug_assert.
        let mut needs_input = !matches!(self.params.store.last_role(), Some(MessageRole::User));

        loop {
            // ── Idle phase ──────────────────────────────────────────
            if needs_input {
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
                            self.notify_observers_user_input();
                        }
                        None => break,
                    },
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
            self.emit_inbox_consumed().await;

            let cancel = TurnCancel::new(self.interrupt.clone(), self.interrupt_tx.clone());
            let mut turn_ctx = TurnContext::new(self.turn_count, cancel);
            // After try_recover, store may end with an Assistant message but
            // turn_ctx is fresh (per-turn lifetime). Re-prime intent so
            // ReadyToCall's invariant holds and Provider::finalize_messages
            // receives the continuation context.
            if !matches!(self.params.store.last_role(), Some(MessageRole::User)) {
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
                        self.emit_interrupted().await?;
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

    fn classify_turn_error(&self, err: &LoopalError) -> ErrorClass {
        match self
            .params
            .deps
            .kernel
            .resolve_provider(self.params.config.model())
        {
            Ok(provider) => provider.classify_error(err),
            Err(_) => default_classify_error(err),
        }
    }

    async fn try_recover(
        &mut self,
        class: ErrorClass,
        server_block_retry: &mut bool,
        context_overflow_retry: &mut bool,
    ) -> Result<bool> {
        match class {
            ErrorClass::ServerBlockError if !*server_block_retry => {
                *server_block_retry = true;
                info!("condensing server blocks after API rejection, retrying");
                self.params.store.condense_server_blocks();
                Ok(true)
            }
            ErrorClass::ContextOverflow if !*context_overflow_retry => {
                *context_overflow_retry = true;
                info!("context overflow detected, emergency compacting and retrying");
                self.params.store.emergency_compact(5);
                self.emit(AgentEventPayload::Error {
                    message: "Context overflow — compacting and retrying...".into(),
                })
                .await?;
                Ok(true)
            }
            // PrefillRejected: provider's finalize_messages should have prevented
            // this. If it leaks here, the model catalog is misconfigured (model
            // marked supports_prefill=true when it isn't). Failing fast surfaces
            // the bug instead of silently retrying without state change.
            _ => Ok(false),
        }
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

    pub async fn emit_inbox_consumed(&mut self) {
        let ids = std::mem::take(&mut self.pending_consumed_ids);
        for message_id in ids {
            let _ = self
                .emit(AgentEventPayload::InboxConsumed { message_id })
                .await;
        }
    }
}
