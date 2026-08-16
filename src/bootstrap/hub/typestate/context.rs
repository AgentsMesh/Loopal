use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_agent_hub::Hub;

pub struct BootstrapContext {
    pub hub: Arc<Mutex<Hub>>,
    pub agent_proc: loopal_agent_client::AgentProcess,
    pub root_session_id: String,
    pub hub_token: String,
    pub(crate) workflow_runtime: Option<loopal_agent_hub::workflow::WorkflowRuntime>,
}

impl BootstrapContext {
    /// Drain workflow workers before closing the root Agent transport.
    ///
    /// All host modes use this consuming path so an actor or ticker cannot
    /// outlive the process that owns its exact execution leases.
    pub async fn shutdown(self) -> anyhow::Result<()> {
        let BootstrapContext {
            hub: _,
            agent_proc,
            root_session_id: _,
            hub_token: _,
            workflow_runtime,
        } = self;

        let workflow_result = match workflow_runtime {
            Some(runtime) => runtime
                .shutdown()
                .await
                .map_err(|error| anyhow::anyhow!("workflow shutdown failed: {error}")),
            None => Ok(()),
        };
        let agent_result = agent_proc
            .shutdown()
            .await
            .map_err(|error| anyhow::anyhow!("root Agent shutdown failed: {error}"));

        combine_shutdown_results(workflow_result, agent_result)
    }
}

fn combine_shutdown_results(
    workflow_result: anyhow::Result<()>,
    agent_result: anyhow::Result<()>,
) -> anyhow::Result<()> {
    match (workflow_result, agent_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(workflow_error), Err(agent_error)) => Err(anyhow::anyhow!(
            "{workflow_error}; additionally, {agent_error}"
        )),
    }
}

pub struct HubAliveInfo {
    pub addr: String,
    pub token: String,
}

#[cfg(test)]
mod tests {
    use super::combine_shutdown_results;

    #[test]
    fn combines_both_shutdown_failures() {
        let error = combine_shutdown_results(
            Err(anyhow::anyhow!("workflow shutdown failed")),
            Err(anyhow::anyhow!("root Agent shutdown failed")),
        )
        .expect_err("both shutdown failures must be reported");

        let message = error.to_string();
        assert!(message.contains("workflow shutdown failed"));
        assert!(message.contains("root Agent shutdown failed"));
    }

    #[test]
    fn returns_each_individual_shutdown_failure() {
        let workflow_error =
            combine_shutdown_results(Err(anyhow::anyhow!("workflow shutdown failed")), Ok(()))
                .expect_err("workflow failure must be returned");
        assert_eq!(workflow_error.to_string(), "workflow shutdown failed");

        let agent_error =
            combine_shutdown_results(Ok(()), Err(anyhow::anyhow!("root Agent shutdown failed")))
                .expect_err("Agent failure must be returned");
        assert_eq!(agent_error.to_string(), "root Agent shutdown failed");
    }
}
