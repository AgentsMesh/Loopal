use std::borrow::Cow;
use std::sync::Arc;

use futures::StreamExt;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::transport::common::client_side_sse::BoxedSseResponse;
use rmcp::transport::streamable_http_client::{StreamableHttpError, StreamableHttpPostResponse};

use crate::oauth_credential_seed::{
    OAUTH_CREDENTIAL_ERROR, OAUTH_RESPONSE_DENIED, OAuthCredentialSeed,
};

pub(super) fn denied<E>(message: &'static str) -> StreamableHttpError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(message))
}

pub(super) fn sanitize_error<E>(error: StreamableHttpError<E>) -> StreamableHttpError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match error {
        StreamableHttpError::ServerDoesNotSupportSse => {
            StreamableHttpError::ServerDoesNotSupportSse
        }
        StreamableHttpError::ServerDoesNotSupportDeleteSession => {
            StreamableHttpError::ServerDoesNotSupportDeleteSession
        }
        StreamableHttpError::SessionExpired => StreamableHttpError::SessionExpired,
        _ => denied(OAUTH_RESPONSE_DENIED),
    }
}

pub(super) fn sanitize_response<E>(
    response: StreamableHttpPostResponse,
    credentials: Arc<OAuthCredentialSeed>,
) -> Result<StreamableHttpPostResponse, StreamableHttpError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match response {
        StreamableHttpPostResponse::Accepted => Ok(StreamableHttpPostResponse::Accepted),
        StreamableHttpPostResponse::Json(mut message, session_id) => {
            reject_secret_session(&session_id, &credentials)?;
            sanitize_message(&mut message, &credentials)?;
            Ok(StreamableHttpPostResponse::Json(message, session_id))
        }
        StreamableHttpPostResponse::Sse(stream, session_id) => {
            reject_secret_session(&session_id, &credentials)?;
            Ok(StreamableHttpPostResponse::Sse(
                sanitize_sse(stream, credentials),
                session_id,
            ))
        }
        _ => Err(denied(OAUTH_RESPONSE_DENIED)),
    }
}

pub(super) fn sanitize_sse(
    stream: BoxedSseResponse,
    credentials: Arc<OAuthCredentialSeed>,
) -> BoxedSseResponse {
    stream
        .filter_map(move |event| {
            let sanitized = event.ok().and_then(|mut event| {
                let redactor = credentials.redactor().ok()?;
                event.event = event.event.map(|value| redactor.scan_and_redact(&value).0);
                event.id = event.id.map(|value| redactor.scan_and_redact(&value).0);
                event.data = event.data.map(|value| sanitize_data(value, &redactor));
                Some(Ok(event))
            });
            futures::future::ready(sanitized)
        })
        .boxed()
}

fn sanitize_message<E>(
    message: &mut ServerJsonRpcMessage,
    credentials: &OAuthCredentialSeed,
) -> Result<(), StreamableHttpError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let redactor = credentials
        .redactor()
        .map_err(|_| denied(OAUTH_CREDENTIAL_ERROR))?;
    let mut value = serde_json::to_value(&*message).map_err(|_| denied(OAUTH_RESPONSE_DENIED))?;
    sanitize_json(&mut value, &redactor);
    *message = serde_json::from_value(value).map_err(|_| denied(OAUTH_RESPONSE_DENIED))?;
    Ok(())
}

fn reject_secret_session<E>(
    session_id: &Option<String>,
    credentials: &OAuthCredentialSeed,
) -> Result<(), StreamableHttpError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if let Some(value) = session_id
        && redact_text(value, credentials)? != *value
    {
        return Err(denied(OAUTH_RESPONSE_DENIED));
    }
    Ok(())
}

fn redact_text<E>(
    value: &str,
    credentials: &OAuthCredentialSeed,
) -> Result<String, StreamableHttpError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    credentials
        .redactor()
        .map(|redactor| redactor.scan_and_redact(value).0)
        .map_err(|_| denied(OAUTH_CREDENTIAL_ERROR))
}

fn sanitize_data(value: String, redactor: &loopal_secret_runtime::Redactor) -> String {
    match serde_json::from_str::<serde_json::Value>(&value) {
        Ok(mut value) => {
            sanitize_json(&mut value, redactor);
            serde_json::to_string(&value).unwrap_or_else(|_| OAUTH_RESPONSE_DENIED.into())
        }
        Err(_) => redactor.scan_and_redact(&value).0,
    }
}

fn sanitize_json(value: &mut serde_json::Value, redactor: &loopal_secret_runtime::Redactor) {
    match value {
        serde_json::Value::String(text) => *text = redactor.scan_and_redact(text).0,
        serde_json::Value::Array(values) => values
            .iter_mut()
            .for_each(|value| sanitize_json(value, redactor)),
        serde_json::Value::Object(values) => {
            let mut sanitized = serde_json::Map::new();
            for (key, mut value) in std::mem::take(values) {
                sanitize_json(&mut value, redactor);
                sanitized.insert(redactor.scan_and_redact(&key).0, value);
            }
            *values = sanitized;
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "oauth_http_sanitize_tests.rs"]
mod tests;
