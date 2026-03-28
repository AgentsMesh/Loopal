//! OAuth flow orchestration for MCP HTTP connections.

use std::sync::Arc;
use std::time::Duration;

use loopal_error::McpError;
use rmcp::transport::WorkerTransport;
use rmcp::transport::auth::{AuthClient, AuthorizationManager, CredentialStore, OAuthState};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransportConfig, StreamableHttpClientWorker,
};
use tracing::{info, warn};

use super::callback;
use super::store::FileCredentialStore;
use crate::client::McpClient;
use crate::handler::SamplingCallback;

/// Attempt an OAuth-authenticated connection to an MCP HTTP server.
///
/// 1. Discover OAuth metadata from the server.
/// 2. Check for cached credentials.
/// 3. If no valid cached token, run browser-based authorization.
/// 4. Connect with AuthClient wrapping the HTTP client.
pub async fn connect_with_oauth(
    url: &str,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
) -> Result<McpClient, McpError> {
    info!(url, "starting OAuth flow for MCP server");

    let mut auth_manager = AuthorizationManager::new(url)
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("OAuth metadata discovery: {e}")))?;

    auth_manager.set_credential_store(FileCredentialStore::new(url));

    // Try cached credentials first.
    match auth_manager.initialize_from_store().await {
        Ok(true) => match auth_manager.refresh_token().await {
            Ok(_) => {
                info!("using cached OAuth credentials");
                return connect_authed(url, auth_manager, timeout, sampling).await;
            }
            Err(e) => {
                warn!(error = %e, "OAuth token refresh failed, re-authorizing");
                let _ = FileCredentialStore::new(url).clear().await;
            }
        },
        Ok(false) => {
            info!("no cached OAuth credentials found");
        }
        Err(e) => {
            warn!(error = %e, "failed to load cached OAuth credentials");
        }
    }

    // Browser-based authorization flow.
    let (port, code_rx) = callback::start_callback_server()
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("callback server: {e}")))?;

    let redirect_uri = format!("http://localhost:{port}/oauth_callback");

    let mut oauth_state = OAuthState::new(url, None)
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("OAuth state init: {e}")))?;

    oauth_state
        .start_authorization(&[], &redirect_uri, Some("loopal"))
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("OAuth start auth: {e}")))?;

    let auth_url = oauth_state
        .get_authorization_url()
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("get auth URL: {e}")))?;

    // Open browser (best-effort).
    info!("opening browser for OAuth authorization");
    if opener::open(auth_url.as_str()).is_err() {
        warn!("could not open browser; authorize manually:\n  {auth_url}");
    }

    // Wait for callback (60s timeout).
    let params = tokio::time::timeout(Duration::from_secs(60), code_rx)
        .await
        .map_err(|_| McpError::Timeout("OAuth callback timeout (60s)".into()))?
        .map_err(|_| McpError::ConnectionFailed("callback channel closed".into()))?;

    oauth_state
        .handle_callback(&params.code, &params.state)
        .await
        .map_err(|e| McpError::ConnectionFailed(format!("token exchange: {e}")))?;

    let mut auth_manager = oauth_state
        .into_authorization_manager()
        .ok_or_else(|| McpError::ConnectionFailed("no authorization manager".into()))?;

    // Persist the obtained credentials for future sessions.
    auth_manager.set_credential_store(FileCredentialStore::new(url));

    connect_authed(url, auth_manager, timeout, sampling).await
}

/// Connect using an already-authorized `AuthorizationManager`.
async fn connect_authed(
    url: &str,
    auth_manager: AuthorizationManager,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
) -> Result<McpClient, McpError> {
    let http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| McpError::ConnectionFailed(format!("HTTP client: {e}")))?;
    let auth_client = AuthClient::new(http_client, auth_manager);
    let config = StreamableHttpClientTransportConfig::with_uri(url);
    let worker = StreamableHttpClientWorker::new(auth_client, config);
    let transport = WorkerTransport::spawn(worker);
    McpClient::connect(transport, timeout, sampling).await
}
