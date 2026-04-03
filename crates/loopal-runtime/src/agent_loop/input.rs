//! Agent input handling — wait for user input, scheduler triggers, and
//! Hub-injected notifications (e.g. sub-agent completion).

use crate::agent_input::AgentInput;
use loopal_error::Result;
use loopal_protocol::{Envelope, MessageSource};
use tracing::{error, info};

use super::WaitResult;
use super::message_build::build_user_message;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Wait for input from any source. Returns None if all channels closed.
    ///
    /// Does NOT emit AwaitingInput — that's handled by `run_loop`'s
    /// state machine via `transition(WaitingForInput)`.
    pub async fn wait_for_input(&mut self) -> Result<Option<WaitResult>> {
        let stale = self.interrupt.take();
        if stale {
            info!("cleared stale interrupt before waiting for input");
        }
        info!("awaiting input");
        loop {
            // Select between frontend input and scheduler triggers.
            // Hub-injected notifications (sub-agent completion) arrive via
            // frontend.recv_input() through the IPC → input_tx path.
            let input = if let Some(ref mut rx) = self.trigger_rx {
                tokio::select! {
                    input = self.params.deps.frontend.recv_input() => input,
                    envelope = rx.recv() => {
                        if let Some(env) = envelope {
                            return Ok(Some(self.ingest_message(&env)));
                        }
                        info!("scheduler channel closed");
                        self.trigger_rx = None;
                        continue;
                    }
                }
            } else {
                self.params.deps.frontend.recv_input().await
            };
            match input {
                Some(AgentInput::Message(env)) => {
                    return Ok(Some(self.ingest_message(&env)));
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

    /// Accept a message envelope: persist (if not ephemeral) and push to store.
    pub(super) fn ingest_message(&mut self, env: &Envelope) -> WaitResult {
        let mut user_msg = build_user_message(env);
        let ephemeral = matches!(
            env.source,
            MessageSource::Scheduled | MessageSource::System(_)
        );
        if !ephemeral
            && let Err(e) = self
                .params
                .deps
                .session_manager
                .save_message(&self.params.session.id, &mut user_msg)
        {
            error!(error = %e, "failed to persist message");
        }
        self.params.store.push_user(user_msg);
        WaitResult::MessageAdded
    }

    /// Non-blocking drain of all pending input (frontend + scheduler).
    /// Returns immediately with whatever messages are queued. Used by Task
    /// agents to check if there's more work before deciding to exit.
    pub(super) async fn drain_pending_input(&mut self) -> Vec<Envelope> {
        let mut pending = self.params.deps.frontend.drain_pending().await;
        // Also drain scheduler triggers.
        if let Some(ref mut rx) = self.trigger_rx {
            while let Ok(env) = rx.try_recv() {
                pending.push(env);
            }
        }
        pending
    }
}
