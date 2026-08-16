use std::sync::Arc;
use std::time::Duration;

use loopal_error::McpError;
use rmcp::service::{RoleClient, ServiceExt};
use tracing::info;

use crate::client::McpClient;
use crate::handler::{LoopalClientHandler, SamplingCallback};
use crate::handshake_transport::{HandshakePolicy, HandshakeSanitizingTransport};

impl McpClient {
    pub async fn connect<T, E, A>(
        transport: T,
        timeout: Duration,
        sampling: Option<Arc<dyn SamplingCallback>>,
    ) -> Result<Self, McpError>
    where
        T: rmcp::transport::IntoTransport<RoleClient, E, A>,
        E: std::error::Error + From<std::io::Error> + Send + Sync + 'static,
    {
        Self::connect_with_policy(transport, timeout, sampling, HandshakePolicy::Strip).await
    }

    pub(crate) async fn connect_with_policy<T, E, A>(
        transport: T,
        timeout: Duration,
        sampling: Option<Arc<dyn SamplingCallback>>,
        policy: HandshakePolicy,
    ) -> Result<Self, McpError>
    where
        T: rmcp::transport::IntoTransport<RoleClient, E, A>,
        E: std::error::Error + From<std::io::Error> + Send + Sync + 'static,
    {
        let handler = LoopalClientHandler::new(sampling);
        let transport = HandshakeSanitizingTransport::new(transport.into_transport(), policy);
        let service = handler.serve(transport).await.map_err(|error| {
            let detail = error.to_string().to_ascii_lowercase();
            if detail.contains("auth") || detail.contains("401") {
                McpError::ConnectionFailed("MCP authentication required".into())
            } else {
                McpError::ConnectionFailed("MCP handshake failed".into())
            }
        })?;

        if let Some(info) = service.peer_info() {
            info!(protocol = ?info.protocol_version, "MCP server connected");
        }
        Ok(Self {
            service,
            timeout,
            oauth_credentials: None,
        })
    }
}
