use std::sync::Arc;
use std::time::Duration;

use loopal_error::McpError;
use rmcp::transport::WorkerTransport;
use rmcp::transport::auth::{AuthClient, AuthorizationManager};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransportConfig, StreamableHttpClientWorker,
};

use crate::client::McpClient;
use crate::handler::SamplingCallback;
use crate::handshake_http_client::HandshakeStrippingHttpClient;
use crate::oauth_credential_seed::OAuthCredentialSeed;
use crate::oauth_http_client::OAuthObservingHttpClient;
use crate::safe_diagnostics::connection_failed;

#[cfg(test)]
#[path = "oauth_transport_tests.rs"]
mod tests;

pub(super) async fn connect(
    url: &str,
    http_client: reqwest::Client,
    auth_manager: AuthorizationManager,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
) -> Result<McpClient, McpError> {
    let credentials = Arc::new(OAuthCredentialSeed::default());
    let http_client = OAuthObservingHttpClient::new(http_client, credentials.clone());
    let auth_client = AuthClient::new(http_client, auth_manager);
    let client = HandshakeStrippingHttpClient::new(auth_client);
    let config = StreamableHttpClientTransportConfig::with_uri(url);
    let worker = StreamableHttpClientWorker::new(client, config);
    let transport = WorkerTransport::spawn(worker);
    match McpClient::connect(transport, timeout, sampling).await {
        Ok(client) => Ok(client.with_oauth_credentials(credentials)),
        Err(_) => {
            credentials.deactivate();
            Err(connection_failed("OAuth MCP connection failed"))
        }
    }
}
