use std::sync::Arc;

use loopal_config::McpServerConfig;
use loopal_secret_client::SecretClient;

use crate::connection::McpConnection;
use crate::handler::SamplingCallback;

pub(super) async fn prepare_connection(
    name: String,
    config: McpServerConfig,
    secret_client: Option<Arc<dyn SecretClient>>,
    sampling: Option<Arc<dyn SamplingCallback>>,
) -> Option<McpConnection> {
    if !config.enabled() {
        tracing::info!(server = %name, "MCP server disabled, skipping");
        return None;
    }
    Some(McpConnection::new(name, config, sampling).with_secret_client(secret_client))
}
