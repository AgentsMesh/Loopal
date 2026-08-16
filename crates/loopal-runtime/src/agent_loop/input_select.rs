use crate::agent_input::AgentInput;

use super::input::SelectResult;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn select_input(&mut self) -> SelectResult {
        match self.try_select_input().await {
            SelectResult::NoInput => {}
            ready => return ready,
        }
        if matches!(self.status, loopal_protocol::AgentStatus::Suspended) {
            return SelectResult::AgentInput(self.params.deps.frontend.recv_input().await);
        }
        match (&mut self.trigger_rx, &mut self.rewake_rx) {
            (Some(scheduled), Some(rewake)) => tokio::select! {
                input = self.params.deps.frontend.recv_input() => SelectResult::AgentInput(input),
                envelope = scheduled.recv() => match envelope {
                    Some(envelope) => SelectResult::Envelope(envelope),
                    None => { self.trigger_rx = None; SelectResult::ChannelClosed }
                },
                envelope = rewake.recv() => match envelope {
                    Some(envelope) => SelectResult::Envelope(envelope),
                    None => { self.rewake_rx = None; SelectResult::ChannelClosed }
                },
            },
            (Some(scheduled), None) => tokio::select! {
                input = self.params.deps.frontend.recv_input() => SelectResult::AgentInput(input),
                envelope = scheduled.recv() => match envelope {
                    Some(envelope) => SelectResult::Envelope(envelope),
                    None => { self.trigger_rx = None; SelectResult::ChannelClosed }
                },
            },
            (None, Some(rewake)) => tokio::select! {
                input = self.params.deps.frontend.recv_input() => SelectResult::AgentInput(input),
                envelope = rewake.recv() => match envelope {
                    Some(envelope) => SelectResult::Envelope(envelope),
                    None => { self.rewake_rx = None; SelectResult::ChannelClosed }
                },
            },
            (None, None) => SelectResult::AgentInput(self.params.deps.frontend.recv_input().await),
        }
    }

    pub(super) async fn try_select_input(&mut self) -> SelectResult {
        let frontend_closed = match self.params.deps.frontend.try_recv_input().await {
            Ok(input @ (AgentInput::Message(_) | AgentInput::WorkflowTerminal(_)))
                if !matches!(self.status, loopal_protocol::AgentStatus::Suspended)
                    && !self.deferred_frontend_inputs.is_empty() =>
            {
                self.deferred_frontend_inputs.push_back(input);
                let deferred = self
                    .deferred_frontend_inputs
                    .pop_front()
                    .expect("deferred queue was checked non-empty");
                return SelectResult::AgentInput(Some(deferred));
            }
            Ok(input) => return SelectResult::AgentInput(Some(input)),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => false,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => true,
        };
        if matches!(self.status, loopal_protocol::AgentStatus::Suspended) {
            return if frontend_closed {
                SelectResult::AgentInput(None)
            } else {
                SelectResult::NoInput
            };
        }
        if let Some(input) = self.deferred_frontend_inputs.pop_front() {
            return SelectResult::AgentInput(Some(input));
        }
        if let Some(envelope) = try_automatic(&mut self.trigger_rx) {
            return SelectResult::Envelope(envelope);
        }
        if let Some(envelope) = try_automatic(&mut self.rewake_rx) {
            return SelectResult::Envelope(envelope);
        }
        if frontend_closed {
            SelectResult::AgentInput(None)
        } else {
            SelectResult::NoInput
        }
    }

    pub(super) fn should_defer_frontend_input(&self, input: &AgentInput) -> bool {
        if !matches!(self.status, loopal_protocol::AgentStatus::Suspended) {
            return false;
        }
        match input {
            AgentInput::Message(envelope) => !envelope.source.wakes_suspended_session(),
            AgentInput::WorkflowTerminal(_) => true,
            AgentInput::Control(_) | AgentInput::TrackedControl(_) => false,
        }
    }
}

fn try_automatic(
    receiver: &mut Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
) -> Option<loopal_protocol::Envelope> {
    let result = receiver.as_mut()?.try_recv();
    match result {
        Ok(envelope) => Some(envelope),
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
            *receiver = None;
            None
        }
        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
    }
}
