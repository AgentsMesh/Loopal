//! IPC Bridge — connects in-process channels (TUI side) to IPC (Agent side).
//!
//! Reuses the Connection from AgentClient (via `into_parts()`) to avoid
//! creating a second reader loop on the same Transport.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope, UserQuestionResponse};

/// Timeout for permission/question responses from TUI (prevents infinite hang).
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);

/// Handles for the TUI side of the IPC bridge.
pub struct BridgeHandles {
    pub agent_event_rx: mpsc::Receiver<AgentEvent>,
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub permission_tx: mpsc::Sender<bool>,
    pub question_tx: mpsc::Sender<UserQuestionResponse>,
    pub mailbox_tx: mpsc::Sender<Envelope>,
}

/// Start the IPC bridge using an existing Connection (from `AgentClient::into_parts()`).
///
/// This avoids creating a second reader loop on the same Transport.
pub fn start_bridge(
    connection: Arc<Connection>,
    incoming_rx: mpsc::Receiver<Incoming>,
) -> BridgeHandles {
    let (agent_event_tx, agent_event_rx) = mpsc::channel::<AgentEvent>(256);
    let (control_tx, mut control_rx) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, mut permission_rx) = mpsc::channel::<bool>(16);
    let (question_tx, mut question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let (mailbox_tx, mut mailbox_rx) = mpsc::channel::<Envelope>(16);

    // Bridge: IPC incoming → TUI events + permission/question response routing
    let conn_in = connection.clone();
    tokio::spawn(async move {
        bridge_incoming(
            incoming_rx,
            conn_in,
            agent_event_tx,
            &mut permission_rx,
            &mut question_rx,
        )
        .await;
    });

    // Bridge: TUI → IPC (control commands)
    let conn_ctrl = connection.clone();
    tokio::spawn(async move {
        while let Some(cmd) = control_rx.recv().await {
            if let Ok(params) = serde_json::to_value(&cmd) {
                if let Err(e) = conn_ctrl
                    .send_request(methods::AGENT_CONTROL.name, params)
                    .await
                {
                    warn!("bridge: control send failed: {e}");
                    break;
                }
            }
        }
    });

    // Bridge: TUI → IPC (mailbox messages)
    let conn_msg = connection.clone();
    tokio::spawn(async move {
        while let Some(envelope) = mailbox_rx.recv().await {
            if let Ok(params) = serde_json::to_value(&envelope) {
                if let Err(e) = conn_msg
                    .send_request(methods::AGENT_MESSAGE.name, params)
                    .await
                {
                    warn!("bridge: message send failed: {e}");
                    break;
                }
            }
        }
    });

    BridgeHandles {
        agent_event_rx,
        control_tx,
        permission_tx,
        question_tx,
        mailbox_tx,
    }
}

async fn bridge_incoming(
    mut incoming_rx: mpsc::Receiver<Incoming>,
    connection: Arc<Connection>,
    event_tx: mpsc::Sender<AgentEvent>,
    permission_rx: &mut mpsc::Receiver<bool>,
    question_rx: &mut mpsc::Receiver<UserQuestionResponse>,
) {
    loop {
        let Some(incoming) = incoming_rx.recv().await else {
            info!("IPC bridge: connection closed");
            break;
        };
        match incoming {
            Incoming::Notification { method, params } => {
                if method == methods::AGENT_EVENT.name {
                    match serde_json::from_value::<AgentEvent>(params) {
                        Ok(event) => {
                            if event_tx.send(event).await.is_err() {
                                warn!("IPC bridge: event channel closed");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("IPC bridge: failed to parse agent event: {e}");
                        }
                    }
                }
            }
            Incoming::Request { id, method, params } => {
                if method == methods::AGENT_PERMISSION.name {
                    handle_permission(&connection, &event_tx, permission_rx, id, params).await;
                } else if method == methods::AGENT_QUESTION.name {
                    handle_question(&connection, &event_tx, question_rx, id, params).await;
                } else {
                    let _ = connection
                        .respond_error(
                            id,
                            loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
                            &format!("unknown: {method}"),
                        )
                        .await;
                }
            }
        }
    }
}

async fn handle_permission(
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
            name: tool_name,
            input: tool_input,
        },
    };
    let _ = event_tx.send(event).await;
    // Wait with timeout — prevents infinite hang if TUI disappears
    let allow = match tokio::time::timeout(RESPONSE_TIMEOUT, permission_rx.recv()).await {
        Ok(Some(v)) => v,
        _ => {
            warn!("permission response timeout/closed, denying");
            false
        }
    };
    let _ = connection
        .respond(request_id, serde_json::json!({"allow": allow}))
        .await;
}

async fn handle_question(
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
