use std::sync::Arc;

use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{AgentEvent, AgentEventPayload, WorkflowRunSnapshot, WorkflowRunSummary};
use tokio::sync::mpsc;

pub(super) mod admission;
mod config;
mod dispatch;
pub(super) mod scheduling;
mod spawn;

pub(super) use config::{WorkflowRuntimeConfig, WorkflowTrustedCeilings};

use super::command::WorkflowCommand;
use super::journal::WorkflowJournalStorage;
use super::state::WorkflowActorState;
use super::{WorkflowClock, WorkflowIdSource};

const WORKFLOW_COMMAND_CAPACITY: usize = 64;

pub(in crate::workflow) struct WorkflowCoordinatorSinks {
    pub(in crate::workflow) event_sink: Option<mpsc::Sender<AgentEvent>>,
    pub(in crate::workflow) terminal_sink: Arc<dyn super::terminal_delivery::WorkflowTerminalSink>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowCoordinatorMode {
    Disabled,
    Preview,
    Execution,
    #[cfg(test)]
    ExecutionHarness,
}

impl WorkflowCoordinatorMode {
    pub(super) fn executes(self) -> bool {
        match self {
            Self::Execution => true,
            #[cfg(test)]
            Self::ExecutionHarness => true,
            Self::Disabled | Self::Preview => false,
        }
    }
}

pub struct WorkflowCoordinator {
    pub(super) mode: WorkflowCoordinatorMode,
    pub(super) clock: Arc<dyn WorkflowClock>,
    ids: Arc<dyn WorkflowIdSource>,
    pub(super) journal: Arc<dyn WorkflowJournalStorage>,
    commands: mpsc::Receiver<WorkflowCommand>,
    pub(super) state: WorkflowActorState,
    spawner: Arc<dyn super::scheduler::WorkflowSpawner>,
    active: super::scheduler::ActiveAttempts,
    pending: super::scheduler::PendingAttempts,
    pub(super) callbacks: mpsc::WeakSender<WorkflowCommand>,
    cancel_grace_ms: u64,
    trusted_ceilings: WorkflowTrustedCeilings,
    pub(super) recovery_grace_ms: u64,
    pub(super) recovery_deadlines:
        std::collections::HashMap<loopal_protocol::WorkflowAttemptId, u64>,
    pub(super) recovered_adoptions: std::collections::HashSet<loopal_protocol::WorkflowAttemptId>,
    pub(super) resumed_owners: std::collections::HashSet<super::WorkflowOwner>,
    pub(super) terminal_deliveries:
        std::collections::HashSet<loopal_protocol::WorkflowTerminalDeliveryId>,
    pub(super) terminal_delivery_payloads: std::collections::HashMap<
        loopal_protocol::WorkflowTerminalDeliveryId,
        loopal_protocol::WorkflowTerminalNotification,
    >,
    pub(super) terminal_delivery_owners: std::collections::HashSet<super::WorkflowOwner>,
    pub(super) terminal_delivery_failure: Option<super::WorkflowCoordinatorError>,
    pub(super) revisions: super::wait::RevisionSenders,
    event_sink: Option<mpsc::Sender<AgentEvent>>,
    pub(super) terminal_sink: Arc<dyn super::terminal_delivery::WorkflowTerminalSink>,
    pub(super) redaction_seed: FinalSinkRedactionSeed,
}

impl WorkflowCoordinator {
    pub(super) fn poison_owner(&mut self, owner: super::WorkflowOwner) {
        self.state.poison(owner.clone());
        if self.mode.executes() {
            self.quarantine_owner(&owner);
        }
    }

    pub(super) fn publish_revision(
        &mut self,
        owner: &super::WorkflowOwner,
        snapshot: &WorkflowRunSnapshot,
    ) {
        if let Some(event_sink) = &self.event_sink {
            let event = AgentEvent::named(
                owner.root_agent.clone(),
                AgentEventPayload::WorkflowRunChanged(WorkflowRunSummary::from(snapshot)),
            );
            let event = self.redaction_seed.guard_event(event);
            if let Err(error) = event_sink.try_send(event) {
                tracing::warn!(
                    root = %owner.root_agent,
                    run_id = %snapshot.id,
                    %error,
                    "workflow projection event dropped; authoritative snapshot remains available"
                );
            }
        }
        super::wait::publish(&mut self.revisions, owner, snapshot);
        super::terminal_delivery::schedule(self, owner, snapshot);
    }

    async fn run(mut self) {
        while let Some(command) = self.commands.recv().await {
            if let WorkflowCommand::Shutdown { response } = command {
                self.commands.close();
                let result = self.drain_scheduler().await;
                let _ = response.send(result);
                return;
            }
            self.dispatch(command).await;
        }
        let _ = self.drain_scheduler().await;
    }
}
