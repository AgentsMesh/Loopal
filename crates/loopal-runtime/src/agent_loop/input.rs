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

pub(super) enum PendingInput {
    Ready(WaitResult),
    Queued(Box<QueuedInput>),
    Empty,
    Closed,
}

pub(super) enum QueuedInput {
    FrontendMessage(Envelope),
    ScheduledEnvelope(Envelope),
}

enum InputProgress {
    Ready(WaitResult),
    Continue,
    Closed,
    Empty,
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
            match self.process_selected_input(input).await? {
                InputProgress::Ready(result) => return Ok(Some(result)),
                InputProgress::Closed => {
                    info!("input channel closed, ending agent loop");
                    return Ok(None);
                }
                InputProgress::Continue | InputProgress::Empty => continue,
            }
        }
    }

    /// Consume real work already queued at a turn boundary. This is the
    /// arbitration point between external input/control and synthetic goal
    /// continuation: queued external work always wins.
    pub(super) async fn poll_pending_input(&mut self) -> Result<PendingInput> {
        loop {
            let input = self.try_select_input().await;
            match input {
                SelectResult::AgentInput(Some(AgentInput::Message(env))) => {
                    if self.should_defer_frontend_message(&env) {
                        self.deferred_frontend_messages.push_back(env);
                        continue;
                    }
                    return Ok(PendingInput::Queued(Box::new(
                        QueuedInput::FrontendMessage(env),
                    )));
                }
                SelectResult::Envelope(env) => {
                    return Ok(PendingInput::Queued(Box::new(
                        QueuedInput::ScheduledEnvelope(env),
                    )));
                }
                other => match self.process_selected_input(other).await? {
                    InputProgress::Ready(result) => return Ok(PendingInput::Ready(result)),
                    InputProgress::Closed => return Ok(PendingInput::Closed),
                    InputProgress::Empty => return Ok(PendingInput::Empty),
                    InputProgress::Continue => {}
                },
            }
        }
    }

    pub(super) async fn consume_queued_input(&mut self, input: Box<QueuedInput>) -> WaitResult {
        match *input {
            QueuedInput::FrontendMessage(env) => {
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
                result
            }
            QueuedInput::ScheduledEnvelope(env) => self.ingest_message(&env).await,
        }
    }

    async fn process_selected_input(&mut self, input: SelectResult) -> Result<InputProgress> {
        match input {
            SelectResult::AgentInput(Some(AgentInput::Message(env))) => {
                if self.should_defer_frontend_message(&env) {
                    self.deferred_frontend_messages.push_back(env);
                    return Ok(InputProgress::Continue);
                }
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
                Ok(InputProgress::Ready(result))
            }
            SelectResult::AgentInput(Some(AgentInput::Control(ctrl))) => {
                if self.apply_untracked_control(ctrl).await? {
                    Ok(InputProgress::Ready(WaitResult::ContinuationInjected))
                } else {
                    Ok(InputProgress::Continue)
                }
            }
            SelectResult::AgentInput(Some(AgentInput::TrackedControl(request))) => {
                if self.apply_tracked_control(request).await? {
                    Ok(InputProgress::Ready(WaitResult::ContinuationInjected))
                } else {
                    Ok(InputProgress::Continue)
                }
            }
            SelectResult::AgentInput(None) => Ok(InputProgress::Closed),
            SelectResult::Envelope(env) => {
                Ok(InputProgress::Ready(self.ingest_message(&env).await))
            }
            SelectResult::ChannelClosed => Ok(InputProgress::Continue),
            SelectResult::NoInput => Ok(InputProgress::Empty),
        }
    }

    async fn select_input(&mut self) -> SelectResult {
        match self.try_select_input().await {
            SelectResult::NoInput => {}
            ready => return ready,
        }
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

    async fn try_select_input(&mut self) -> SelectResult {
        let frontend_closed = match self.params.deps.frontend.try_recv_input().await {
            Ok(AgentInput::Message(env))
                if !matches!(self.status, loopal_protocol::AgentStatus::Suspended)
                    && !self.deferred_frontend_messages.is_empty() =>
            {
                // Controls still win at a boundary, while newly arrived data
                // stays behind envelopes deferred earlier during suspension.
                self.deferred_frontend_messages.push_back(env);
                let deferred = self
                    .deferred_frontend_messages
                    .pop_front()
                    .expect("deferred queue was checked non-empty");
                return SelectResult::AgentInput(Some(AgentInput::Message(deferred)));
            }
            Ok(input) => return SelectResult::AgentInput(Some(input)),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => false,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => true,
        };

        // Suspend gates every automatic source. Only frontend input is polled
        // here so a human message or Unsuspend control can reopen the session;
        // scheduled and hook re-wake envelopes stay queued until then.
        if matches!(self.status, loopal_protocol::AgentStatus::Suspended) {
            return if frontend_closed {
                SelectResult::AgentInput(None)
            } else {
                SelectResult::NoInput
            };
        }

        if let Some(env) = self.deferred_frontend_messages.pop_front() {
            return SelectResult::AgentInput(Some(AgentInput::Message(env)));
        }

        if let Some(rx) = &mut self.trigger_rx {
            match rx.try_recv() {
                Ok(env) => return SelectResult::Envelope(env),
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    self.trigger_rx = None;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &mut self.rewake_rx {
            match rx.try_recv() {
                Ok(env) => return SelectResult::Envelope(env),
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    self.rewake_rx = None;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            }
        }

        if frontend_closed {
            SelectResult::AgentInput(None)
        } else {
            SelectResult::NoInput
        }
    }

    pub(super) async fn drain_pending_input(&mut self) -> Vec<Envelope> {
        let all_inputs = self.params.deps.frontend.drain_pending().await;
        let mut pending: Vec<_> = self.deferred_frontend_messages.drain(..).collect();
        for input in all_inputs {
            match input {
                AgentInput::Message(env) => pending.push(env),
                AgentInput::Control(cmd) => {
                    if let Err(e) = self.apply_untracked_control(cmd).await {
                        tracing::warn!(error = %e, "failed to handle drained control");
                    }
                }
                AgentInput::TrackedControl(request) => {
                    if let Err(e) = self.apply_tracked_control(request).await {
                        tracing::warn!(error = %e, "failed to handle tracked control");
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

    fn should_defer_frontend_message(&self, env: &Envelope) -> bool {
        matches!(self.status, loopal_protocol::AgentStatus::Suspended)
            && !env.source.wakes_suspended_session()
    }
}

enum SelectResult {
    AgentInput(Option<AgentInput>),
    Envelope(Envelope),
    ChannelClosed,
    NoInput,
}
