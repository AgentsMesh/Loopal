//! Request handlers for the IPC bridge (permission + question flows).

use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{debug, warn};

use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentEvent, UserQuestionResponse};

/// Timeout for permission/question responses from consumer (prevents infinite hang).
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) async fn handle_permission(
    connection: &Connection,
    event_tx: &mpsc::Sender<AgentEvent>,
    permission_rx: &mut mpsc::Receiver<bool>,
    request_id: i64,
    params: serde_json::Value,
) {
    let tool_name = params["tool_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let tool_input = params.get("tool_input").cloned().unwrap_or_default();
    let tool_id = params["tool_call_id"].as_str().unwrap_or("").to_string();
    let event = AgentEvent {
        agent_name: None,
        payload: loopal_protocol::AgentEventPayload::ToolPermissionRequest {
            id: tool_id,
            name: tool_name.clone(),
            input: tool_input,
        },
    };
    let _ = event_tx.send(event).await;
    // Wait with timeout — prevents infinite hang if consumer disappears
    let allow = match tokio::time::timeout(RESPONSE_TIMEOUT, permission_rx.recv()).await {
        Ok(Some(v)) => {
            debug!(tool = %tool_name, allow = v, "bridge: permission response");
            v
        }
        _ => {
            warn!(tool = %tool_name, "permission response timeout/closed, denying");
            false
        }
    };
    let _ = connection
        .respond(request_id, serde_json::json!({"allow": allow}))
        .await;
}

pub(crate) async fn handle_question(
    connection: &Connection,
    event_tx: &mpsc::Sender<AgentEvent>,
    question_rx: &mut mpsc::Receiver<UserQuestionResponse>,
    request_id: i64,
    params: serde_json::Value,
) {
    let parsed = serde_json::from_value(params.get("questions").cloned().unwrap_or_default());
    if let Ok(questions) = parsed {
        let event = AgentEvent {
            agent_name: None,
            payload: loopal_protocol::AgentEventPayload::UserQuestionRequest {
                id: "ipc".into(),
                questions,
            },
        };
        let _ = event_tx.send(event).await;
    } else {
        // Parse failed — respond immediately instead of waiting 300s
        warn!("IPC bridge: failed to parse questions, auto-responding");
        let fallback = UserQuestionResponse {
            answers: vec!["(parse error)".into()],
        };
        let _ = connection
            .respond(
                request_id,
                serde_json::to_value(&fallback).unwrap_or_default(),
            )
            .await;
        return;
    }
    let response = match tokio::time::timeout(RESPONSE_TIMEOUT, question_rx.recv()).await {
        Ok(Some(v)) => v,
        _ => {
            warn!("question response timeout/closed");
            UserQuestionResponse {
                answers: vec!["(timeout)".into()],
            }
        }
    };
    let _ = connection
        .respond(
            request_id,
            serde_json::to_value(&response).unwrap_or_default(),
        )
        .await;
}
