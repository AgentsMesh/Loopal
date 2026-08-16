use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use loopal_runtime::agent_input::AgentInput;

use crate::session_hub::SharedSession;

pub(super) async fn route_request(
    id: i64,
    method: &str,
    params: serde_json::Value,
    session: &SharedSession,
    connection: &Connection<Listening>,
) {
    match method {
        m if m == methods::AGENT_INTERRUPT.name => {
            signal_interrupt(session);
            let _ = connection
                .respond(id, serde_json::json!({"ok": true}))
                .await;
        }
        m if m == methods::AGENT_MESSAGE.name => match serde_json::from_value::<Envelope>(params) {
            Ok(env) => match session.input_tx.send(AgentInput::Message(env)).await {
                Ok(()) => {
                    let _ = connection
                        .respond(id, serde_json::json!({"ok": true}))
                        .await;
                }
                Err(_) => {
                    tracing::warn!("agent/message request reached a closed session input channel");
                    let _ = connection
                        .respond_error(
                            id,
                            jsonrpc::INTERNAL_ERROR,
                            "session input channel is closed",
                        )
                        .await;
                }
            },
            Err(error) => {
                let _ = connection
                    .respond_error(id, jsonrpc::INVALID_REQUEST, &error.to_string())
                    .await;
            }
        },
        m if m == methods::AGENT_STATE_SNAPSHOT.name => {
            let snapshot = session
                .snapshot_agent_state()
                .await
                .unwrap_or_else(loopal_protocol::AgentStateSnapshot::empty);
            match serde_json::to_value(&snapshot) {
                Ok(payload) => {
                    let _ = connection.respond(id, payload).await;
                }
                Err(error) => {
                    let _ = connection
                        .respond_error(id, jsonrpc::INTERNAL_ERROR, &error.to_string())
                        .await;
                }
            }
        }
        _ => {
            let _ = connection
                .respond_error(id, jsonrpc::METHOD_NOT_FOUND, &format!("unknown: {method}"))
                .await;
        }
    }
}

pub(super) fn signal_interrupt(session: &SharedSession) {
    session.interrupt.signal();
    session
        .interrupt_tx
        .send_modify(|value| *value = value.wrapping_add(1));
}

/// Routes control and shared requests for an observer connection.
/// Only used in integration tests (production no longer has agent/join).
#[allow(dead_code)]
pub(crate) async fn observer_loop(
    incoming_rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    connection: &Arc<Connection<Listening>>,
    session: &Arc<SharedSession>,
    client_id: &str,
) {
    while let Some(msg) = incoming_rx.recv().await {
        match msg {
            Incoming::Request { id, method, params } => {
                if method == methods::AGENT_CONTROL.name {
                    crate::control_forward::spawn(
                        id,
                        params,
                        Arc::clone(session),
                        Arc::clone(connection),
                    );
                } else {
                    route_request(id, &method, params, session, connection).await;
                }
            }
            Incoming::Notification { method, .. } => {
                if method == methods::AGENT_INTERRUPT.name {
                    signal_interrupt(session);
                }
            }
        }
    }
    session.remove_client(client_id).await;
}
