use std::sync::Arc;

use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_storage::SessionStore;
use tokio::sync::Mutex;

use crate::Hub;
use crate::spawn_manager::ProductionWorkflowSpawner;

use super::super::actor::{
    WorkflowCoordinatorSinks, WorkflowRuntimeConfig, WorkflowTrustedCeilings,
};
use super::super::journal::SessionWorkflowJournals;
use super::super::terminal_delivery::HubWorkflowTerminalSink;
use super::super::{
    SystemWorkflowClock, SystemWorkflowIdSource, WorkflowCoordinator, WorkflowCoordinatorMode,
};
#[cfg(test)]
use super::DROP_CLEANUP_TIMEOUT;
use super::{WorkflowRuntime, WorkflowRuntimeError};

impl WorkflowRuntime {
    /// Build the execution backend when workflow tools are enabled by the
    /// exact same predicate used by the root agent.
    ///
    /// The returned runtime has not yet been installed in the Hub. This keeps
    /// workflow RPC authorization closed until durable recovery succeeds.
    pub async fn new_production(
        hub: Arc<Mutex<Hub>>,
        sessions: Arc<SessionStore>,
        settings: &WorkflowSettings,
    ) -> Result<Option<Self>, WorkflowRuntimeError> {
        if !settings.execution_enabled || settings.policy == OrchestrationPolicy::Off {
            return Ok(None);
        }
        settings
            .validate()
            .map_err(WorkflowRuntimeError::InvalidSettings)?;

        let (event_sender, redaction_seed, shutdown_signal) = {
            let hub = hub.lock().await;
            if hub.protected_audit.is_none() {
                return Err(WorkflowRuntimeError::ProtectedAuditUnavailable);
            }
            (
                hub.registry.event_sender(),
                hub.final_sink_redaction_seed(),
                hub.shutdown_signal.clone(),
            )
        };
        let spawner = ProductionWorkflowSpawner::new(hub.clone(), shutdown_signal.clone());
        let runtime_config = WorkflowRuntimeConfig {
            ceilings: WorkflowTrustedCeilings::from_settings(settings),
            cancel_grace_ms: settings.timing.cancel_grace_secs.saturating_mul(1_000),
            recovery_grace_ms: settings.timing.recovery_grace_secs.saturating_mul(1_000),
            redaction_seed,
        };
        let journal_seed = runtime_config.redaction_seed.clone();
        let terminal_sink = Arc::new(HubWorkflowTerminalSink::new(hub.clone()));
        let (handle, actor_task) = WorkflowCoordinator::spawn_with_runtime_config_and_sinks(
            WorkflowCoordinatorMode::Execution,
            Arc::new(SystemWorkflowClock),
            Arc::new(SystemWorkflowIdSource),
            Arc::new(SessionWorkflowJournals::new_with_redaction_seed(
                sessions,
                journal_seed,
            )),
            spawner,
            runtime_config,
            WorkflowCoordinatorSinks {
                event_sink: Some(event_sender),
                terminal_sink,
            },
        );
        Ok(Some(Self {
            hub,
            shutdown_signal,
            handle,
            actor_task: Some(actor_task),
            ticker: None,
            admitted: false,
            owner: None,
            #[cfg(test)]
            drop_cleanup_timeout: DROP_CLEANUP_TIMEOUT,
            #[cfg(test)]
            drop_cleanup_probe: None,
        }))
    }
}
