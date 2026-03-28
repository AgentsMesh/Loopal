//! Message forwarding while an agent session is active.
//!
//! Routes incoming IPC messages to the session's input channel and signals
//! interrupts. Returns when agent completes or a new agent/start arrives.

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;
use loopal_protocol::{ControlCommand, Envelope};

use crate::session_hub::InputFromClient;
use crate::session_start::SessionHandle;

/// Result of forward_loop — tells dispatch_loop what happened.
pub(crate) enum ForwardResult {
    /// Agent completed or connection closed.
    Done,
    /// A new agent/start request arrived during active session.
    NewStart { id: i64, params: serde_json::Value },
}

/// Forward messages from the connection to the active session.
pub(crate) async fn forward_loop(
    incoming_rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    connection: &Connection,
    handle: &mut SessionHandle,
) -> ForwardResult {
    let session = &handle.session;

    loop {
        tokio::select! {
            msg = incoming_rx.recv() => {
                let Some(msg) = msg else {
                    return ForwardResult::Done;
                };
                match msg {
                    Incoming::Request { id, method, params } => {
                        if method == methods::AGENT_START.name {
                            // New session requested — stop current, return pending
                            session.interrupt.signal();
                            session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                            let _ = (&mut handle.agent_task).await;
                            return ForwardResult::NewStart { id, params };
                        }
                        route_request(id, &method, params, session, connection).await;
                    }
                    Incoming::Notification { method, .. } => {
                        if method == methods::AGENT_INTERRUPT.name {
                            session.interrupt.signal();
                            session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                        }
                    }
                }
            }
            _ = &mut handle.agent_task => {
                return ForwardResult::Done;
            }
        }
    }
}

async fn route_request(
    id: i64,
    method: &str,
    params: serde_json::Value,
    session: &crate::session_hub::SharedSession,
    connection: &Connection,
) {
    match method {
        m if m == methods::AGENT_MESSAGE.name => match serde_json::from_value::<Envelope>(params) {
            Ok(env) => {
                let _ = session.input_tx.send(InputFromClient::Message(env)).await;
                let _ = connection
                    .respond(id, serde_json::json!({"ok": true}))
                    .await;
            }
            Err(e) => {
                let _ = connection
                    .respond_error(id, jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
            }
        },
        m if m == methods::AGENT_CONTROL.name => {
            match serde_json::from_value::<ControlCommand>(params) {
                Ok(cmd) => {
                    let _ = session.input_tx.send(InputFromClient::Control(cmd)).await;
                    let _ = connection
                        .respond(id, serde_json::json!({"ok": true}))
                        .await;
                }
                Err(e) => {
                    let _ = connection
                        .respond_error(id, jsonrpc::INVALID_REQUEST, &e.to_string())
                        .await;
                }
            }
        }
        m if m == methods::AGENT_SHUTDOWN.name => {
            session.interrupt.signal();
            let _ = connection
                .respond(id, serde_json::json!({"ok": true}))
                .await;
        }
        _ => {
            let _ = connection
                .respond_error(id, jsonrpc::METHOD_NOT_FOUND, &format!("unknown: {method}"))
                .await;
        }
    }
}

/// Observer loop: joined client receives events via HubFrontend broadcast.
/// Only used in integration tests (production no longer has agent/join).
#[allow(dead_code)]
pub(crate) async fn observer_loop(
    incoming_rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    connection: &Connection,
    session: &crate::session_hub::SharedSession,
    client_id: &str,
) {
    while let Some(msg) = incoming_rx.recv().await {
        match msg {
            Incoming::Request { id, method, params } => {
                route_request(id, &method, params, session, connection).await;
            }
            Incoming::Notification { method, .. } => {
                if method == methods::AGENT_INTERRUPT.name {
                    session.interrupt.signal();
                    session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                }
            }
        }
    }
    session.remove_client(client_id).await;
}
