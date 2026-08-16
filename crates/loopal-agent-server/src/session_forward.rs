//! Message forwarding while an agent session is active.
//!
//! Routes incoming IPC messages to the session's input channel and signals
//! interrupts. Returns when agent completes or a new agent/start arrives.

use std::sync::Arc;
use std::time::Duration;

use loopal_error::AgentOutput;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use loopal_runtime::agent_input::AgentInput;

use crate::session_start::SessionHandle;

mod routing;

#[allow(unused_imports)]
pub(crate) use routing::observer_loop;
use routing::{route_request, signal_interrupt};

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
                        if method == methods::AGENT_WORKFLOW_TERMINAL.name {
                            crate::workflow_terminal_forward::spawn(
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

#[cfg(test)]
#[path = "session_forward/tests.rs"]
mod tests;
