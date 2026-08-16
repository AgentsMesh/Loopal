use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use reqwest::header::{HeaderName, HeaderValue};
use rmcp::model::{ClientJsonRpcMessage, ClientRequest, RequestId, ServerJsonRpcMessage};
use rmcp::transport::common::client_side_sse::BoxedSseResponse;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};

use crate::handshake_transport::{HandshakePolicy, sanitize_handshake_message};
use crate::oauth_http_sanitize::sanitize_error as sanitize_oauth_error;

const HANDSHAKE_ERROR: &str = "MCP handshake sanitization unavailable";

#[derive(Clone)]
pub(crate) struct HandshakeStrippingHttpClient<C> {
    inner: C,
}

impl<C> HandshakeStrippingHttpClient<C> {
    pub(crate) fn new(inner: C) -> Self {
        Self { inner }
    }
}

impl<C> StreamableHttpClient for HandshakeStrippingHttpClient<C>
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
        let request_id = initialize_request_id(&message);
        let response = self
            .inner
            .post_message(uri, message, session_id, auth_header, custom_headers)
            .await;
        match (response, request_id) {
            (Ok(response), Some(request_id)) => {
                sanitize_response(response, HandshakePolicy::Strip, request_id)
            }
            (Err(error), Some(_)) => Err(sanitize_initial_error(error)),
            (Ok(response), None) => Ok(response),
            (Err(error), None) => Err(sanitize_oauth_error(error)),
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        self.inner
            .delete_session(uri, session_id, auth_header, custom_headers)
            .await
            .map_err(sanitize_oauth_error)
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxedSseResponse, StreamableHttpError<Self::Error>> {
        self.inner
            .get_stream(uri, session_id, last_event_id, auth_header, custom_headers)
            .await
            .map_err(sanitize_oauth_error)
    }
}

pub(crate) fn initialize_request_id(message: &ClientJsonRpcMessage) -> Option<RequestId> {
    match message {
        ClientJsonRpcMessage::Request(request)
            if matches!(request.request, ClientRequest::InitializeRequest(_)) =>
        {
            Some(request.id.clone())
        }
        _ => None,
    }
}

pub(crate) fn sanitize_response<E>(
    response: StreamableHttpPostResponse,
    policy: HandshakePolicy,
    request_id: RequestId,
) -> Result<StreamableHttpPostResponse, StreamableHttpError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match response {
        StreamableHttpPostResponse::Json(mut message, session_id) => {
            reject_secret_session(&session_id, &policy)?;
            if !sanitize_handshake_message(&mut message, &policy, Some(&request_id)) {
                return Err(handshake_error());
            }
            Ok(StreamableHttpPostResponse::Json(message, session_id))
        }
        StreamableHttpPostResponse::Sse(stream, session_id) => {
            reject_secret_session(&session_id, &policy)?;
            Ok(StreamableHttpPostResponse::Sse(
                sanitize_sse(stream, policy, request_id),
                session_id,
            ))
        }
        _ => Err(handshake_error()),
    }
}

fn sanitize_sse(
    stream: BoxedSseResponse,
    policy: HandshakePolicy,
    request_id: RequestId,
) -> BoxedSseResponse {
    stream
        .filter_map(move |event| {
            let sanitized = match event {
                Ok(mut event) => event
                    .data
                    .as_deref()
                    .and_then(|data| serde_json::from_str::<ServerJsonRpcMessage>(data).ok())
                    .and_then(|mut message| {
                        sanitize_handshake_message(&mut message, &policy, Some(&request_id))
                            .then(|| serde_json::to_string(&message).ok())
                            .flatten()
                    })
                    .map(|data| {
                        event.data = Some(data);
                        event.event = None;
                        event.id = None;
                        event.retry = None;
                        Ok(event)
                    }),
                Err(error) => Some(Err(error)),
            };
            futures::future::ready(sanitized)
        })
        .boxed()
}

fn reject_secret_session<E>(
    session_id: &Option<String>,
    policy: &HandshakePolicy,
) -> Result<(), StreamableHttpError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if session_id
        .as_deref()
        .is_some_and(|value| !policy.accepts_opaque_text(value))
    {
        return Err(handshake_error());
    }
    Ok(())
}

pub(crate) fn sanitize_initial_error<E>(error: StreamableHttpError<E>) -> StreamableHttpError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match error {
        StreamableHttpError::AuthRequired(_) | StreamableHttpError::InsufficientScope(_) => {
            StreamableHttpError::UnexpectedServerResponse("MCP authentication required".into())
        }
        _ => handshake_error(),
    }
}

fn handshake_error<E>() -> StreamableHttpError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    StreamableHttpError::UnexpectedServerResponse(HANDSHAKE_ERROR.into())
}

#[cfg(test)]
#[path = "handshake_http_client_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "handshake_http_client_tests.rs"]
mod tests;
