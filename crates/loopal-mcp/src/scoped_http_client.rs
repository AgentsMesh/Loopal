use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::McpServerConfig;
use loopal_secret_client::SecretClient;
use reqwest::header::{HeaderName, HeaderValue};
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::common::client_side_sse::BoxedSseResponse;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};

use crate::handshake_http_client::{
    initialize_request_id, sanitize_initial_error, sanitize_response,
};
use crate::handshake_transport::HandshakePolicy;
use crate::resolved_config::ResolvedMcpConfig;
use crate::secret_expand::{CONFIG_SECRET_ERROR, expand_mcp_config, resolve_bound_mcp_secret_seed};
use crate::secret_provenance::SecretProvenance;

#[derive(Clone)]
pub(crate) struct ScopedHttpClient {
    client: reqwest::Client,
    config: McpServerConfig,
    secret_client: Option<Arc<dyn SecretClient>>,
    secret_provenance: Arc<SecretProvenance>,
}

impl ScopedHttpClient {
    pub(crate) fn new(
        config: McpServerConfig,
        secret_client: Option<Arc<dyn SecretClient>>,
        secret_provenance: Arc<SecretProvenance>,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(crate::http_redirect_policy::same_origin())
                .build()
                .expect("MCP HTTP client initialization"),
            config,
            secret_client,
            secret_provenance,
        }
    }

    async fn handshake_policy(
        &self,
    ) -> Result<HandshakePolicy, StreamableHttpError<reqwest::Error>> {
        let seed = resolve_bound_mcp_secret_seed(
            &self.config,
            self.secret_client.as_ref(),
            &self.secret_provenance,
            loopal_ipc::HUB_RPC_BUDGET,
        )
        .await
        .map_err(|_| config_error())?;
        Ok(HandshakePolicy::from_seed(&seed))
    }

    async fn request_headers(
        &self,
    ) -> Result<HashMap<HeaderName, HeaderValue>, StreamableHttpError<reqwest::Error>> {
        let ResolvedMcpConfig::StreamableHttp { headers, .. } = expand_mcp_config(
            self.config.clone(),
            self.secret_client.as_ref(),
            &self.secret_provenance,
            loopal_ipc::HUB_RPC_BUDGET,
        )
        .await
        .map_err(|_| config_error())?
        else {
            return Err(config_error());
        };
        headers
            .into_iter()
            .map(|(key, value)| {
                let name = HeaderName::from_bytes(key.as_bytes()).map_err(|_| config_error())?;
                let value = HeaderValue::from_str(&value).map_err(|_| config_error())?;
                Ok((name, value))
            })
            .collect()
    }
}

impl StreamableHttpClient for ScopedHttpClient {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        mut custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let request_id = initialize_request_id(&message);
        let policy = match request_id.as_ref() {
            Some(_) => Some(self.handshake_policy().await?),
            None => None,
        };
        custom_headers.extend(self.request_headers().await?);
        let response = self
            .client
            .post_message(uri, message, session_id, auth_header, custom_headers)
            .await;
        match (response, policy, request_id) {
            (Ok(response), Some(policy), Some(request_id)) => {
                sanitize_response(response, policy, request_id)
            }
            (Err(error), Some(_), Some(_)) => Err(sanitize_initial_error(error)),
            (Ok(response), _, _) => Ok(response),
            (Err(error), _, _) => Err(error),
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_header: Option<String>,
        mut custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        custom_headers.extend(self.request_headers().await?);
        self.client
            .delete_session(uri, session_id, auth_header, custom_headers)
            .await
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        mut custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxedSseResponse, StreamableHttpError<Self::Error>> {
        custom_headers.extend(self.request_headers().await?);
        self.client
            .get_stream(uri, session_id, last_event_id, auth_header, custom_headers)
            .await
    }
}

fn config_error() -> StreamableHttpError<reqwest::Error> {
    StreamableHttpError::UnexpectedServerResponse(CONFIG_SECRET_ERROR.into())
}

#[cfg(test)]
#[path = "scoped_http_client_handshake_tests.rs"]
mod handshake_tests;

#[cfg(test)]
#[path = "scoped_http_client_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scoped_http_redirect_tests.rs"]
mod redirect_tests;
