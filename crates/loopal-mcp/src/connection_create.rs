use std::time::Duration;

use loopal_config::McpServerConfig;
use loopal_secret_client::SecretString;

use crate::client::McpClient;
use crate::connection::McpConnection;
use crate::handshake_transport::HandshakePolicy;
use crate::resolved_config::ResolvedMcpConfig;
use crate::secret_expand::resolve_bound_mcp_secret_seed;
use crate::transport;

impl McpConnection {
    async fn resolve_bound_seed(
        &self,
    ) -> Result<Vec<(String, SecretString)>, loopal_error::McpError> {
        resolve_bound_mcp_secret_seed(
            &self.config,
            self.secret_client.as_ref(),
            &self.secret_provenance,
            loopal_ipc::HUB_RPC_BUDGET,
        )
        .await
        .map_err(|message| loopal_error::McpError::Protocol(message.into()))
    }

    pub(super) async fn create_client(
        &self,
        timeout: Duration,
    ) -> Result<McpClient, loopal_error::McpError> {
        let sampling = self.sampling.clone();
        let seed = self.resolve_bound_seed().await?;
        match &self.config {
            McpServerConfig::Stdio { .. } => {
                let ResolvedMcpConfig::Stdio { command, args, env } =
                    ResolvedMcpConfig::from_config(self.config.clone(), &seed)
                else {
                    unreachable!()
                };
                transport::connect_stdio_with_policy(
                    &command,
                    &args,
                    &env,
                    timeout,
                    sampling,
                    Some(self.stderr_tail.clone()),
                    HandshakePolicy::from_seed(&seed),
                )
                .await
            }
            McpServerConfig::StreamableHttp { url, .. } => {
                let client = crate::scoped_http_client::ScopedHttpClient::new(
                    self.config.clone(),
                    self.secret_client.clone(),
                    self.secret_provenance.clone(),
                );
                transport::connect_http(
                    url,
                    client,
                    timeout,
                    sampling,
                    HandshakePolicy::from_seed(&seed),
                )
                .await
            }
        }
    }
}
