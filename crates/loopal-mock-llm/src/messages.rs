use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::http::{HttpRequest, write_json, write_sse_head};
use crate::protocol::{WireProtocol, authenticate};
use crate::state::ServerState;
use crate::{SseAction, plan_protocol_sse, validate_request};

pub(crate) async fn handle_completion(
    stream: &mut TcpStream,
    request: HttpRequest,
    state: &ServerState,
    protocol: WireProtocol,
    route_model: Option<String>,
) -> Result<()> {
    let auth = match authenticate(&request, protocol, state.api_key()) {
        Ok(value) => value,
        Err(error) => {
            return write_json(stream, error.status, &error.body, &BTreeMap::new()).await;
        }
    };
    let body: Value = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => {
            return write_json(
                stream,
                400,
                &json!({"error": "invalid JSON request"}),
                &BTreeMap::new(),
            )
            .await;
        }
    };
    let (mut record, call) = state
        .take_call(
            protocol,
            &body,
            route_model.as_deref(),
            auth.key_present,
            auth.version_present,
        )
        .await;
    let Some(call) = call else {
        record.matched = false;
        state.record(record).await;
        return write_json(
            stream,
            409,
            &json!({"error": "scenario has no matching response"}),
            &BTreeMap::new(),
        )
        .await;
    };
    let errors = validate_request(&call.expected, &body, &record);
    record.matched = errors.is_empty();
    state.record(record).await;
    if !errors.is_empty() {
        return write_json(
            stream,
            409,
            &json!({"error": "request expectation failed", "details": errors}),
            &BTreeMap::new(),
        )
        .await;
    }
    let _response = state.begin_response();
    if call.response.close_before_headers {
        state.record_scripted_disconnect();
        return Ok(());
    }
    if call.response.delay_ms > 0
        && disconnect_during(stream, Duration::from_millis(call.response.delay_ms)).await
    {
        state.record_client_disconnect();
        return Ok(());
    }
    if call.response.status != 200 {
        return write_scripted_error(stream, call.response).await;
    }
    let actions = plan_protocol_sse(protocol, &call.response)?;
    write_sse_head(stream, &call.response.headers).await?;
    let mut events = 0usize;
    for action in actions {
        match action {
            SseAction::Delay(duration) => {
                if disconnect_during(stream, duration).await {
                    state.record_client_disconnect();
                    return Ok(());
                }
            }
            SseAction::Event(data) => {
                if stream.write_all(data.as_bytes()).await.is_err() || stream.flush().await.is_err()
                {
                    state.record_client_disconnect();
                    return Ok(());
                }
                events += 1;
                if call.response.disconnect_after_events == Some(events) {
                    state.record_scripted_disconnect();
                    return Ok(());
                }
            }
            SseAction::Disconnect => {
                state.record_scripted_disconnect();
                return Ok(());
            }
        }
    }
    Ok(())
}

async fn disconnect_during(stream: &mut TcpStream, duration: Duration) -> bool {
    let mut probe = [0u8; 1];
    tokio::select! {
        _ = tokio::time::sleep(duration) => false,
        _ = stream.read(&mut probe) => true,
    }
}

async fn write_scripted_error(stream: &mut TcpStream, response: crate::MockResponse) -> Result<()> {
    let mut headers = response.headers;
    if let Some(milliseconds) = response.retry_after_ms {
        headers.insert(
            "retry-after".into(),
            format!("{}", milliseconds as f64 / 1000.0),
        );
    }
    let body = response.body.unwrap_or_else(|| {
        json!({
            "error": {"type": "mock_error", "message": "scripted provider failure"}
        })
    });
    write_json(stream, response.status, &body, &headers).await
}
