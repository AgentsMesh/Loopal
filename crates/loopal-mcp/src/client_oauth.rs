use std::sync::Arc;
use std::time::Duration;

use crate::client::McpClient;
use crate::oauth_credential_seed::OAuthCredentialSeed;

impl McpClient {
    pub(crate) fn with_oauth_credentials(mut self, credentials: Arc<OAuthCredentialSeed>) -> Self {
        self.oauth_credentials = Some(credentials);
        self
    }

    pub(crate) fn oauth_credentials(&self) -> Option<&Arc<OAuthCredentialSeed>> {
        self.oauth_credentials.as_ref()
    }

    pub(crate) async fn close(&mut self, timeout: Duration) {
        let _ = self.service.close_with_timeout(timeout).await;
    }
}
