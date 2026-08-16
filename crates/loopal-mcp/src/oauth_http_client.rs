use std::collections::HashMap;
use std::sync::Arc;

use reqwest::header::{HeaderName, HeaderValue};
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::common::client_side_sse::BoxedSseResponse;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};

use crate::oauth_credential_seed::{OAUTH_CREDENTIAL_ERROR, OAuthCredentialSeed};
use crate::oauth_http_sanitize::{denied, sanitize_error, sanitize_response, sanitize_sse};

#[derive(Clone)]
pub(crate) struct OAuthObservingHttpClient<C> {
    inner: C,
    credentials: Arc<OAuthCredentialSeed>,
}

impl<C> OAuthObservingHttpClient<C> {
    pub(crate) fn new(inner: C, credentials: Arc<OAuthCredentialSeed>) -> Self {
        Self { inner, credentials }
    }

    fn observe<E>(&self, token: Option<&str>) -> Result<(), StreamableHttpError<E>>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        self.credentials
            .observe(token)
            .map_err(|_| denied(OAUTH_CREDENTIAL_ERROR))
    }
}

impl<C> StreamableHttpClient for OAuthObservingHttpClient<C>
where
    C: StreamableHttpClient + Sync,
{
    type Error = C::Error;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.observe(auth_header.as_deref())?;
        let response = self
            .inner
            .post_message(uri, message, session_id, auth_header, custom_headers)
            .await
            .map_err(sanitize_error)?;
        sanitize_response(response, self.credentials.clone())
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        self.observe(auth_header.as_deref())?;
        self.inner
            .delete_session(uri, session_id, auth_header, custom_headers)
            .await
            .map_err(sanitize_error)
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxedSseResponse, StreamableHttpError<Self::Error>> {
        self.observe(auth_header.as_deref())?;
        let stream = self
            .inner
            .get_stream(uri, session_id, last_event_id, auth_header, custom_headers)
            .await
            .map_err(sanitize_error)?;
        Ok(sanitize_sse(stream, self.credentials.clone()))
    }
}

#[cfg(test)]
#[path = "oauth_http_client_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "oauth_http_client_tests.rs"]
mod tests;
