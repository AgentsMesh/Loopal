use std::time::Duration;

use loopal_agent_hub::workflow::{WorkflowOwner, WorkflowRuntime};

use super::states::{Ready, RootPending};

// reason: layered timeouts let the innermost failure surface first.
// proxy(HUB_RPC_BUDGET=8s) < start_agent(20s) < HANDSHAKE(30s).
const START_AGENT_TIMEOUT: Duration = Duration::from_secs(20);

impl RootPending {
    pub fn hub(&self) -> &std::sync::Arc<tokio::sync::Mutex<loopal_agent_hub::Hub>> {
        &self.hub
    }

    pub async fn start_root_agent(
        self,
        mut params: loopal_agent_client::StartAgentParams,
    ) -> anyhow::Result<Ready> {
        let RootPending {
            hub,
            hub_token,
            agent_proc,
            client_conn,
            mut workflow_runtime,
        } = self;
        let expected_session_id = params
            .resume
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if params.resume.is_none() {
            params.session_id = Some(expected_session_id.clone());
        }
        if let Err(error) = loopal_agent_hub::agent_io::bind_managed_root_session_id(
            &hub,
            &client_conn,
            &expected_session_id,
        )
        .await
        {
            shutdown_start_failure(workflow_runtime.take(), agent_proc).await;
            anyhow::bail!(error);
        }

        if let Some(runtime) = workflow_runtime.as_mut() {
            let owner = WorkflowOwner::new(
                expected_session_id.clone(),
                loopal_protocol::QualifiedAddress::local(loopal_protocol::ROOT_AGENT_NAME),
            );
            if let Err(error) = runtime.recover_and_admit(owner).await {
                let runtime = workflow_runtime.take();
                shutdown_start_failure(runtime, agent_proc).await;
                anyhow::bail!("workflow recovery failed: {error}");
            }
        }

        let root_session_id = match loopal_agent_client::AgentClient::start_agent_on(
            &client_conn,
            &params,
            START_AGENT_TIMEOUT,
        )
        .await
        {
            Ok(session_id) => session_id,
            Err(error) => {
                shutdown_start_failure(workflow_runtime.take(), agent_proc).await;
                return Err(error);
            }
        };
        if root_session_id != expected_session_id {
            shutdown_start_failure(workflow_runtime.take(), agent_proc).await;
            anyhow::bail!("root Agent returned a different Hub-bound session id");
        }
        if let Some(runtime) = workflow_runtime.as_ref()
            && let Err(error) = runtime.activate_terminal_deliveries().await
        {
            let runtime = workflow_runtime.take();
            shutdown_start_failure(runtime, agent_proc).await;
            anyhow::bail!("workflow terminal delivery activation failed: {error}");
        }
        drop(client_conn);
        Ok(Ready {
            hub,
            hub_token,
            agent_proc,
            root_session_id,
            workflow_runtime,
        })
    }
}

async fn shutdown_start_failure(
    workflow_runtime: Option<WorkflowRuntime>,
    agent_proc: loopal_agent_client::AgentProcess,
) {
    if let Some(runtime) = workflow_runtime
        && let Err(error) = runtime.shutdown().await
    {
        tracing::warn!(%error, "workflow runtime shutdown failed during bootstrap rollback");
    }
    if let Err(error) = agent_proc.shutdown().await {
        tracing::warn!(%error, "root Agent shutdown failed during bootstrap rollback");
    }
}
