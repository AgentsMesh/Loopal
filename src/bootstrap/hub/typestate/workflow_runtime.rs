use std::sync::Arc;

use loopal_agent_hub::workflow::WorkflowRuntime;
use loopal_storage::SessionStore;

use super::states::RootPending;

impl RootPending {
    /// Construct the production workflow owner while admission is still
    /// closed. The runtime remains in the typestate until root session binding
    /// and durable recovery complete.
    pub async fn install_workflow_runtime(
        self,
        settings: &loopal_config::WorkflowSettings,
    ) -> anyhow::Result<Self> {
        let RootPending {
            hub,
            hub_token,
            agent_proc,
            client_conn,
            workflow_runtime,
        } = self;

        let workflow_runtime = if workflow_runtime.is_some() {
            workflow_runtime
        } else if production_workflow_execution_enabled(settings) {
            let sessions = Arc::new(SessionStore::new()?);
            match WorkflowRuntime::new_production(hub.clone(), sessions, settings).await {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = agent_proc.shutdown().await;
                    return Err(anyhow::anyhow!(
                        "workflow runtime initialization failed: {error}"
                    ));
                }
            }
        } else {
            None
        };

        Ok(Self {
            hub,
            hub_token,
            agent_proc,
            client_conn,
            workflow_runtime,
        })
    }
}

fn production_workflow_execution_enabled(settings: &loopal_config::WorkflowSettings) -> bool {
    settings.execution_enabled && settings.policy != loopal_config::OrchestrationPolicy::Off
}

#[cfg(test)]
#[path = "workflow_runtime_tests.rs"]
mod tests;
