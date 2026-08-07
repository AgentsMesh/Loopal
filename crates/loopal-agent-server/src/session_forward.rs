//! Message forwarding while an agent session is active.
//!
//! Routes incoming IPC messages to the session's input channel and signals
//! interrupts. Returns when agent completes or a new agent/start arrives.

use std::sync::Arc;
use std::time::Duration;

use loopal_error::AgentOutput;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use loopal_runtime::agent_input::AgentInput;

use crate::session_start::SessionHandle;

/// Result of forward_loop — tells dispatch_loop what happened.
pub(crate) enum ForwardResult {
    /// Agent completed or connection closed. Carries the agent's output (if any).
    Done(Option<AgentOutput>),
    /// The peer requested process shutdown while this session was active.
    Shutdown,
    /// A new agent/start request arrived during active session.
    NewStart { id: i64, params: serde_json::Value },
}

/// Forward messages from the connection to the active session.
pub(crate) async fn forward_loop(
    incoming_rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    connection: &Arc<Connection<Listening>>,
    handle: &mut SessionHandle,
) -> ForwardResult {
    let session = &handle.session;

    loop {
        tokio::select! {
            msg = incoming_rx.recv() => {
                let Some(msg) = msg else {
                    // Connection closed (EOF). Signal agent to exit cleanly.
                    session.interrupt.signal();
                    session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                    handle.shutdown.cancel();
                    // Brief wait; if agent didn't exit, re-signal to cover the
                    // race where it consumed the first interrupt during turn
                    // teardown before re-entering recv_input().
                    if tokio::time::timeout(
                        Duration::from_millis(100),
                        &mut handle.agent_task,
                    ).await.is_err() {
                        session.interrupt.signal();
                        session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                        if tokio::time::timeout(
                            Duration::from_millis(900),
                            &mut handle.agent_task,
                        ).await.is_err() {
                            handle.agent_task.abort();
                        }
                    }
                    return ForwardResult::Done(None);
                };
                match msg {
                    Incoming::Request { id, method, params } => {
                        if method == methods::AGENT_START.name {
                            // New session requested — stop current, return pending
                            session.interrupt.signal();
                            session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                            handle.shutdown.cancel();
                            let _ = (&mut handle.agent_task).await;
                            return ForwardResult::NewStart { id, params };
                        }
                        if method == methods::AGENT_SHUTDOWN.name {
                            // `agent/shutdown` terminates the server, not merely
                            // the current turn. Signal first so the ACK proves
                            // cancellation is observable, then propagate the
                            // shutdown disposition to `run_connection`.
                            signal_interrupt(session);
                            handle.shutdown.cancel();
                            let _ = connection
                                .respond(id, serde_json::json!({"ok": true}))
                                .await;
                            return ForwardResult::Shutdown;
                        }
                        if method == methods::AGENT_CONTROL.name {
                            crate::control_forward::spawn(
                                id,
                                params,
                                Arc::clone(session),
                                Arc::clone(connection),
                            );
                            continue;
                        }
                        route_request(id, &method, params, session, connection).await;
                    }
                    Incoming::Notification { method, params } => {
                        if method == methods::AGENT_INTERRUPT.name {
                            tracing::info!("forward_loop: received agent/interrupt, signaling");
                            session.interrupt.signal();
                            session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                        } else if method == methods::AGENT_MESSAGE.name {
                            // Hub-injected message (e.g. sub-agent completion notification).
                            if let Ok(env) = serde_json::from_value::<Envelope>(params)
                                && session.input_tx.send(AgentInput::Message(env)).await.is_err()
                            {
                                tracing::warn!(
                                    "dropping agent/message notification because the session input channel is closed"
                                );
                            }
                        }
                    }
                }
            }
            result = &mut handle.agent_task => {
                let output = result.ok().flatten();
                return ForwardResult::Done(output);
            }
        }
    }
}

async fn route_request(
    id: i64,
    method: &str,
    params: serde_json::Value,
    session: &crate::session_hub::SharedSession,
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
            Err(e) => {
                let _ = connection
                    .respond_error(id, jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
            }
        },
        m if m == methods::AGENT_STATE_SNAPSHOT.name => {
            // Hub-side ViewState rebuild: dump tasks/crons/bg_tasks.
            // Empty payload if no agent is bound yet (placeholder window)
            // or if the agent has already exited (Weak ref upgraded fail).
            let snapshot = session
                .snapshot_agent_state()
                .await
                .unwrap_or_else(loopal_protocol::AgentStateSnapshot::empty);
            match serde_json::to_value(&snapshot) {
                Ok(payload) => {
                    let _ = connection.respond(id, payload).await;
                }
                Err(e) => {
                    let _ = connection
                        .respond_error(id, jsonrpc::INTERNAL_ERROR, &e.to_string())
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

fn signal_interrupt(session: &crate::session_hub::SharedSession) {
    session.interrupt.signal();
    session
        .interrupt_tx
        .send_modify(|value| *value = value.wrapping_add(1));
}

/// Observer loop: joined client receives events via HubFrontend broadcast.
/// Only used in integration tests (production no longer has agent/join).
#[allow(dead_code)]
pub(crate) async fn observer_loop(
    incoming_rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    connection: &Arc<Connection<Listening>>,
    session: &Arc<crate::session_hub::SharedSession>,
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
                    session.interrupt.signal();
                    session.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                }
            }
        }
    }
    session.remove_client(client_id).await;
}
#[cfg(test)]
#[path = "session_forward/interrupt_tests.rs"]
mod interrupt_tests;
