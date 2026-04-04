//! Agent input handling — wait for user input, scheduler triggers,
//! hook rewake signals, and Hub-injected notifications.

use crate::agent_input::AgentInput;
use loopal_error::Result;
use loopal_protocol::{Envelope, MessageSource};
use tracing::{error, info};

use super::WaitResult;
use super::message_build::build_user_message;
use super::runner::AgentLoopRunner;
use crate::fire_hooks::fire_hooks;

impl AgentLoopRunner {
    /// Wait for input from any source. Returns None if all channels closed.
    pub async fn wait_for_input(&mut self) -> Result<Option<WaitResult>> {
        let stale = self.interrupt.take();
        if stale {
            info!("cleared stale interrupt before waiting for input");
        }
        info!("awaiting input");
        loop {
            let input = self.select_input().await;
            match input {
                SelectResult::AgentInput(Some(AgentInput::Message(env))) => {
                    let result = self.ingest_message(&env);
                    fire_hooks(
                        &self.params.deps.kernel,
                        loopal_config::HookEvent::PostInput,
                        &loopal_hooks::HookContext {
                            session_id: Some(&self.params.session.id),
                            ..Default::default()
                        },
                    )
                    .await;
                    return Ok(Some(result));
                }
                SelectResult::AgentInput(Some(AgentInput::Control(ctrl))) => {
                    self.handle_control(ctrl).await?;
                }
                SelectResult::AgentInput(None) => {
                    info!("input channel closed, ending agent loop");
                    return Ok(None);
                }
                SelectResult::Envelope(env) => {
                    return Ok(Some(self.ingest_message(&env)));
                }
                SelectResult::ChannelClosed => continue,
            }
        }
    }

    /// Multiplex frontend, scheduler, and hook rewake channels.
    async fn select_input(&mut self) -> SelectResult {
        match (&mut self.trigger_rx, &mut self.rewake_rx) {
            (Some(sched), Some(rewake)) => tokio::select! {
                input = self.params.deps.frontend.recv_input() => SelectResult::AgentInput(input),
                env = sched.recv() => match env {
                    Some(e) => SelectResult::Envelope(e),
                    None => { self.trigger_rx = None; SelectResult::ChannelClosed }
                },
                env = rewake.recv() => match env {
                    Some(e) => SelectResult::Envelope(e),
                    None => { self.rewake_rx = None; SelectResult::ChannelClosed }
                },
            },
            (Some(sched), None) => tokio::select! {
                input = self.params.deps.frontend.recv_input() => SelectResult::AgentInput(input),
                env = sched.recv() => match env {
                    Some(e) => SelectResult::Envelope(e),
                    None => { self.trigger_rx = None; SelectResult::ChannelClosed }
                },
            },
            (None, Some(rewake)) => tokio::select! {
                input = self.params.deps.frontend.recv_input() => SelectResult::AgentInput(input),
                env = rewake.recv() => match env {
                    Some(e) => SelectResult::Envelope(e),
                    None => { self.rewake_rx = None; SelectResult::ChannelClosed }
                },
            },
            (None, None) => {
                SelectResult::AgentInput(self.params.deps.frontend.recv_input().await)
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

    /// Non-blocking drain of all pending input (frontend + scheduler + rewake).
    pub(super) async fn drain_pending_input(&mut self) -> Vec<Envelope> {
        let mut pending = self.params.deps.frontend.drain_pending().await;
        if let Some(ref mut rx) = self.trigger_rx {
            while let Ok(env) = rx.try_recv() {
                pending.push(env);
            }
        }
        if let Some(ref mut rx) = self.rewake_rx {
            while let Ok(env) = rx.try_recv() {
                pending.push(env);
            }
        }
        pending
    }
}

enum SelectResult {
    AgentInput(Option<AgentInput>),
    Envelope(Envelope),
    ChannelClosed,
}
