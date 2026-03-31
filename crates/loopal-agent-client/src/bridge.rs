//! IPC Bridge — connects in-process channels (consumer side) to IPC (Agent side).
//!
//! Reuses the Connection from AgentClient (via `into_parts()`) to avoid
//! creating a second reader loop on the same Transport.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope, UserQuestionResponse};

use crate::bridge_handlers::{handle_permission, handle_question};

/// Handles for the consumer side of the IPC bridge.
pub struct BridgeHandles {
    pub agent_event_rx: mpsc::Receiver<AgentEvent>,
    pub agent_event_tx: mpsc::Sender<AgentEvent>,
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
    let agent_event_tx_clone = agent_event_tx.clone();
    let (control_tx, mut control_rx) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, mut permission_rx) = mpsc::channel::<bool>(16);
    let (question_tx, mut question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let (mailbox_tx, mut mailbox_rx) = mpsc::channel::<Envelope>(16);

    // Bridge: IPC incoming → consumer events + permission/question response routing
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

    // Bridge: consumer → IPC (control commands)
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

    // Bridge: consumer → IPC (mailbox messages)
    let conn_msg = connection.clone();
    tokio::spawn(async move {
        while let Some(envelope) = mailbox_rx.recv().await {
            debug!(target_agent = %envelope.target, "bridge: forwarding message");
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
        agent_event_tx: agent_event_tx_clone,
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
                debug!(id, %method, "bridge: incoming request");
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
