use crate::agent_input::AgentInput;
use loopal_error::Result;
use loopal_protocol::Envelope;
use tracing::info;

use super::runner::AgentLoopRunner;
use crate::fire_hooks::fire_hooks;

#[derive(Debug)]
pub enum WaitResult {
    MessageAdded,
    WorkflowResultAdded,
    /// The current envelope was handed to the workflow authority, so this
    /// envelope must not enter the ordinary provider/tool execution path.
    WorkflowHandled,
    /// Workflow dispatch may have committed externally, but its outcome is
    /// indeterminate or failed. Direct execution remains forbidden.
    WorkflowFailed(String),
    // reason: distinguish system-injected continuation (handle_control)
    // from fresh user input so callers (e.g. interrupt clearing) can branch.
    // Observers learn the envelope source directly via on_envelope_received.
    ContinuationInjected,
}

impl WaitResult {
    pub(super) fn blocks_provider(&self) -> bool {
        matches!(self, Self::WorkflowHandled | Self::WorkflowFailed(_))
    }

    pub(super) fn into_workflow_failure(self) -> Option<String> {
        match self {
            Self::WorkflowFailed(error) => Some(error),
            _ => None,
        }
    }
}

pub(super) enum PendingInput {
    Ready(WaitResult),
    Queued(Box<QueuedInput>),
    Empty,
    Closed,
}

pub(super) enum QueuedInput {
    Frontend(AgentInput),
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
                SelectResult::AgentInput(Some(input @ AgentInput::Message(_)))
                | SelectResult::AgentInput(Some(input @ AgentInput::WorkflowTerminal(_))) => {
                    if self.should_defer_frontend_input(&input) {
                        self.deferred_frontend_inputs.push_back(input);
                        continue;
                    }
                    return Ok(PendingInput::Queued(Box::new(QueuedInput::Frontend(input))));
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
            QueuedInput::Frontend(input) => self.consume_frontend_data(input).await,
            QueuedInput::ScheduledEnvelope(env) => self.ingest_message(&env).await,
        }
    }

    async fn process_selected_input(&mut self, input: SelectResult) -> Result<InputProgress> {
        match input {
            SelectResult::AgentInput(Some(AgentInput::Message(env))) => {
                let input = AgentInput::Message(env);
                if self.should_defer_frontend_input(&input) {
                    self.deferred_frontend_inputs.push_back(input);
                    return Ok(InputProgress::Continue);
                }
                let AgentInput::Message(env) = input else {
                    unreachable!()
                };
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
            SelectResult::AgentInput(Some(AgentInput::WorkflowTerminal(request))) => {
                let input = AgentInput::WorkflowTerminal(request);
                if self.should_defer_frontend_input(&input) {
                    self.deferred_frontend_inputs.push_back(input);
                    return Ok(InputProgress::Continue);
                }
                let AgentInput::WorkflowTerminal(request) = input else {
                    unreachable!()
                };
                if self.apply_workflow_terminal(request).await {
                    Ok(InputProgress::Ready(WaitResult::WorkflowResultAdded))
                } else {
                    Ok(InputProgress::Continue)
                }
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
}

pub(super) enum SelectResult {
    AgentInput(Option<AgentInput>),
    Envelope(Envelope),
    ChannelClosed,
    NoInput,
}

#[cfg(test)]
#[path = "input_coverage_tests/mod.rs"]
mod coverage_tests;
