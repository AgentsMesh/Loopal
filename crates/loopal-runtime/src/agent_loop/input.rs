use crate::agent_input::AgentInput;
use loopal_error::Result;
use loopal_protocol::Envelope;
use tracing::info;

use super::runner::AgentLoopRunner;
use crate::fire_hooks::fire_hooks;

#[derive(Debug)]
pub enum WaitResult {
    MessageAdded,
    // reason: distinguish system-injected continuation (handle_control)
    // from fresh user input so callers (e.g. interrupt clearing) can branch.
    // Observers learn the envelope source directly via on_envelope_received.
    ContinuationInjected,
}

impl AgentLoopRunner {
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
                    let result = self.ingest_message(&env).await;
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
                    if self.handle_control(ctrl).await? {
                        return Ok(Some(WaitResult::ContinuationInjected));
                    }
                }
                SelectResult::AgentInput(None) => {
                    info!("input channel closed, ending agent loop");
                    return Ok(None);
                }
                SelectResult::Envelope(env) => {
                    return Ok(Some(self.ingest_message(&env).await));
                }
                SelectResult::ChannelClosed => continue,
            }
        }
    }

    async fn select_input(&mut self) -> SelectResult {
        if matches!(self.status, loopal_protocol::AgentStatus::Suspended) {
            return SelectResult::AgentInput(self.params.deps.frontend.recv_input().await);
        }
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
            (None, None) => SelectResult::AgentInput(self.params.deps.frontend.recv_input().await),
        }
    }

    pub(super) async fn drain_pending_input(&mut self) -> Vec<Envelope> {
        let all_inputs = self.params.deps.frontend.drain_pending().await;
        let mut pending = Vec::new();
        for input in all_inputs {
            match input {
                AgentInput::Message(env) => pending.push(env),
                AgentInput::Control(cmd) => {
                    if let Err(e) = self.handle_control(cmd).await {
                        tracing::warn!(error = %e, "failed to handle drained control");
                    }
                }
            }
        }
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
