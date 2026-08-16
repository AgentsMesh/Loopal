use crate::agent_input::AgentInput;

use super::input::WaitResult;
use super::runner::AgentLoopRunner;
use crate::fire_hooks::fire_hooks;

impl AgentLoopRunner {
    pub(super) async fn drain_pending_input(&mut self) -> Vec<loopal_protocol::Envelope> {
        let mut pending = Vec::new();
        let mut inputs: Vec<_> = self.deferred_frontend_inputs.drain(..).collect();
        inputs.extend(self.params.deps.frontend.drain_pending().await);
        for input in inputs {
            match input {
                AgentInput::Message(envelope) => pending.push(envelope),
                AgentInput::Control(command) => {
                    if let Err(error) = self.apply_untracked_control(command).await {
                        tracing::warn!(%error, "failed to handle drained control");
                    }
                }
                AgentInput::TrackedControl(request) => {
                    if let Err(error) = self.apply_tracked_control(request).await {
                        tracing::warn!(%error, "failed to handle tracked control");
                    }
                }
                AgentInput::WorkflowTerminal(request) => {
                    let _ = self.apply_workflow_terminal(request).await;
                }
            }
        }
        drain_envelopes(&mut self.trigger_rx, &mut pending);
        drain_envelopes(&mut self.rewake_rx, &mut pending);
        pending
    }

    pub(super) async fn consume_frontend_data(&mut self, input: AgentInput) -> WaitResult {
        match input {
            AgentInput::Message(envelope) => {
                let result = self.ingest_message(&envelope).await;
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
            AgentInput::WorkflowTerminal(request) => {
                if self.apply_workflow_terminal(request).await {
                    WaitResult::WorkflowResultAdded
                } else {
                    WaitResult::WorkflowHandled
                }
            }
            AgentInput::Control(_) | AgentInput::TrackedControl(_) => {
                unreachable!("control is applied before queued input")
            }
        }
    }
}

fn drain_envelopes(
    receiver: &mut Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    pending: &mut Vec<loopal_protocol::Envelope>,
) {
    if let Some(receiver) = receiver {
        while let Ok(envelope) = receiver.try_recv() {
            pending.push(envelope);
        }
    }
}
