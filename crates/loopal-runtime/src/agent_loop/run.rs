use loopal_error::{AgentOutput, LoopalError, Result, TerminateReason};
use loopal_protocol::{AgentEventPayload, AgentStatus};
use loopal_provider_api::MessageRole;
use loopal_provider_api::{
    ContinuationIntent, ContinuationReason, ErrorClass, default_classify_error,
};
use tracing::{error, info, warn};

pub const CONTEXT_OVERFLOW_BANNER: &str = "Context overflow — compacting and retrying...";

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
                self.turns
                    .with_wire_mut(loopal_context::condense_server_blocks);
                Ok(true)
            }
            ErrorClass::ContextOverflow if !*context_overflow_retry => {
                *context_overflow_retry = true;
                info!("context overflow detected, force-compacting and retrying");
                // Emit banner ONLY after compact succeeded — pre-fix showed
                // "retrying" then errored when force_compact silently bailed.
                let compacted = self.force_compact(None).await?;
                if !compacted {
                    warn!("force_compact declined; ContextOverflow propagates");
                    return Ok(false);
                }
                self.emit_cosmetic(AgentEventPayload::Error {
                    message: CONTEXT_OVERFLOW_BANNER.into(),
                })
                .await;
                Ok(true)
            }
            // PrefillRejected here means provider's finalize_messages let it
            // through — the model catalog has supports_prefill=true wrongly.
            // Fail fast: silent retry would loop without state change.
            _ => Ok(false),
        }
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
