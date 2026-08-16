use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::{
    WORKFLOW_COMMAND_CAPACITY, WorkflowCoordinator, WorkflowCoordinatorMode,
    WorkflowCoordinatorSinks, WorkflowRuntimeConfig,
};
use crate::workflow::journal::{
    SessionWorkflowJournals, UnavailableWorkflowJournals, WorkflowJournalStorage,
};
use crate::workflow::scheduler::{UnavailableWorkflowSpawner, WorkflowSpawner};
use crate::workflow::terminal_delivery::UnavailableWorkflowTerminalSink;
use crate::workflow::{
    SystemWorkflowClock, SystemWorkflowIdSource, WorkflowClock, WorkflowCoordinatorHandle,
    WorkflowIdSource,
};

impl WorkflowCoordinator {
    pub fn spawn_disabled() -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_dependencies(
            WorkflowCoordinatorMode::Disabled,
            Arc::new(SystemWorkflowClock),
            Arc::new(SystemWorkflowIdSource),
            Arc::new(UnavailableWorkflowJournals),
            Arc::new(UnavailableWorkflowSpawner),
        )
    }

    pub fn spawn_disabled_with_storage(
        sessions: Arc<loopal_storage::SessionStore>,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_dependencies(
            WorkflowCoordinatorMode::Disabled,
            Arc::new(SystemWorkflowClock),
            Arc::new(SystemWorkflowIdSource),
            Arc::new(SessionWorkflowJournals::new(sessions)),
            Arc::new(UnavailableWorkflowSpawner),
        )
    }

    pub(crate) fn spawn_with_dependencies(
        mode: WorkflowCoordinatorMode,
        clock: Arc<dyn WorkflowClock>,
        ids: Arc<dyn WorkflowIdSource>,
        journal: Arc<dyn WorkflowJournalStorage>,
        spawner: Arc<dyn WorkflowSpawner>,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_dependencies_and_events(mode, clock, ids, journal, spawner, None)
    }

    pub(crate) fn spawn_with_dependencies_and_events(
        mode: WorkflowCoordinatorMode,
        clock: Arc<dyn WorkflowClock>,
        ids: Arc<dyn WorkflowIdSource>,
        journal: Arc<dyn WorkflowJournalStorage>,
        spawner: Arc<dyn WorkflowSpawner>,
        event_sink: Option<mpsc::Sender<loopal_protocol::AgentEvent>>,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_runtime_config(
            mode,
            clock,
            ids,
            journal,
            spawner,
            event_sink,
            WorkflowRuntimeConfig::test_default(),
        )
    }

    pub(in crate::workflow) fn spawn_with_runtime_config(
        mode: WorkflowCoordinatorMode,
        clock: Arc<dyn WorkflowClock>,
        ids: Arc<dyn WorkflowIdSource>,
        journal: Arc<dyn WorkflowJournalStorage>,
        spawner: Arc<dyn WorkflowSpawner>,
        event_sink: Option<mpsc::Sender<loopal_protocol::AgentEvent>>,
        runtime: WorkflowRuntimeConfig,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_runtime_config_and_sinks(
            mode,
            clock,
            ids,
            journal,
            spawner,
            runtime,
            WorkflowCoordinatorSinks {
                event_sink,
                terminal_sink: Arc::new(UnavailableWorkflowTerminalSink),
            },
        )
    }

    pub(in crate::workflow) fn spawn_with_runtime_config_and_sinks(
        mode: WorkflowCoordinatorMode,
        clock: Arc<dyn WorkflowClock>,
        ids: Arc<dyn WorkflowIdSource>,
        journal: Arc<dyn WorkflowJournalStorage>,
        spawner: Arc<dyn WorkflowSpawner>,
        runtime: WorkflowRuntimeConfig,
        sinks: WorkflowCoordinatorSinks,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        let (commands, receiver) = mpsc::channel(WORKFLOW_COMMAND_CAPACITY);
        let coordinator = Self {
            mode,
            clock,
            ids,
            journal,
            commands: receiver,
            state: super::WorkflowActorState::new(),
            spawner,
            active: Default::default(),
            pending: Default::default(),
            callbacks: commands.downgrade(),
            cancel_grace_ms: runtime.cancel_grace_ms,
            trusted_ceilings: runtime.ceilings,
            recovery_grace_ms: runtime.recovery_grace_ms,
            recovery_deadlines: Default::default(),
            recovered_adoptions: Default::default(),
            resumed_owners: Default::default(),
            terminal_deliveries: Default::default(),
            terminal_delivery_payloads: Default::default(),
            terminal_delivery_owners: Default::default(),
            terminal_delivery_failure: None,
            revisions: Default::default(),
            event_sink: sinks.event_sink,
            terminal_sink: sinks.terminal_sink,
            redaction_seed: runtime.redaction_seed,
        };
        let handle = WorkflowCoordinatorHandle {
            commands: commands.clone(),
        };
        let task = tokio::spawn(coordinator.run());
        (handle, task)
    }

    #[cfg(test)]
    pub(crate) fn spawn_for_test(
        mode: WorkflowCoordinatorMode,
        clock: Arc<dyn WorkflowClock>,
        ids: Arc<dyn WorkflowIdSource>,
        journal: Arc<dyn WorkflowJournalStorage>,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_dependencies(
            mode,
            clock,
            ids,
            journal,
            Arc::new(UnavailableWorkflowSpawner),
        )
    }

    #[cfg(test)]
    pub(in crate::workflow) fn spawn_for_test_with_spawner(
        mode: WorkflowCoordinatorMode,
        clock: Arc<dyn WorkflowClock>,
        ids: Arc<dyn WorkflowIdSource>,
        journal: Arc<dyn WorkflowJournalStorage>,
        spawner: Arc<dyn WorkflowSpawner>,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_dependencies(mode, clock, ids, journal, spawner)
    }

    #[cfg(test)]
    pub(in crate::workflow) fn spawn_for_test_with_events(
        mode: WorkflowCoordinatorMode,
        clock: Arc<dyn WorkflowClock>,
        ids: Arc<dyn WorkflowIdSource>,
        journal: Arc<dyn WorkflowJournalStorage>,
        event_sink: mpsc::Sender<loopal_protocol::AgentEvent>,
    ) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
        Self::spawn_with_dependencies_and_events(
            mode,
            clock,
            ids,
            journal,
            Arc::new(UnavailableWorkflowSpawner),
            Some(event_sink),
        )
    }
}
