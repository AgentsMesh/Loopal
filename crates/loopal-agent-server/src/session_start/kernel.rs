use std::path::Path;
use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};

use crate::params::StartParams;
use crate::session_hub::SessionHub;

pub(super) async fn build(
    connection: &Arc<Connection<Listening>>,
    config: &loopal_config::ResolvedConfig,
    start: &StartParams,
    cwd: &Path,
    session_id: &str,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<Arc<loopal_kernel::Kernel>> {
    let depth = start.depth.unwrap_or(0);
    let agent_name = if depth == 0 {
        loopal_protocol::ROOT_AGENT_NAME.to_string()
    } else {
        "sub".to_string()
    };
    if is_production {
        let workflow_provider = match (
            start.workflow_permission_causation.clone(),
            start.workflow_attempt_capability.clone(),
        ) {
            (Some(causation), Some(capability)) => {
                Some(crate::params::WorkflowProviderSecretAuthority {
                    causation,
                    capability,
                })
            }
            (None, None) => None,
            _ => anyhow::bail!(
                "workflow attempt capability and permission causation must be supplied together"
            ),
        };
        let hub_client: Arc<dyn loopal_mcp::HubMcpClient> = Arc::new(
            crate::connection_mcp_client::ConnectionMcpClient::new(connection.clone()),
        );
        return crate::params::build_kernel_from_config_with_workflow_provider(
            config,
            true,
            depth,
            Some(hub_client),
            Some(connection.clone()),
            cwd.to_path_buf(),
            agent_name,
            session_id.to_string(),
            workflow_provider,
        )
        .await;
    }
    match hub.get_test_provider().await {
        Some(provider) => {
            crate::params::build_kernel_with_provider(provider, start.model.as_deref())
        }
        None => {
            crate::params::build_kernel_from_config(
                config,
                false,
                depth,
                None,
                None,
                cwd.to_path_buf(),
                agent_name,
                session_id.to_string(),
            )
            .await
        }
    }
}

#[cfg(test)]
#[path = "kernel_tests.rs"]
mod tests;
