//! Shared agent IO loop — handles hub/* requests, forwards events,
//! and relays permission/question requests to TUI.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::dispatch::dispatch_hub_request;
use crate::hub::AgentHub;
use crate::tui_relay::relay_to_tui;

/// Method name for hub/wait_agent — must not block the IO loop.
const WAIT_AGENT_METHOD: &str = "hub/wait_agent";

/// Run the IO loop for a connected agent. Returns the agent's final output
/// (from AttemptCompletion or last stream text) for passing to wait_agent watchers.
pub async fn agent_io_loop(
    hub: Arc<Mutex<AgentHub>>,
    conn: Arc<Connection>,
    mut rx: tokio::sync::mpsc::Receiver<Incoming>,
    agent_name: String,
    is_root: bool,
) -> Option<String> {
    info!(agent = %agent_name, is_root, "agent IO loop started");
    let mut last_stream = String::new();
    let mut completion_output: Option<String> = None;

    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Notification { method, params } => {
                if method == methods::AGENT_COMPLETED.name {
                    // Explicit completion — primary signal (EOF is fallback).
                    info!(agent = %agent_name, "received agent/completed");
                    break;
                } else if method == methods::AGENT_EVENT.name {
                    if let Ok(mut event) = serde_json::from_value::<AgentEvent>(params) {
                        // Track output for wait_agent result
                        match &event.payload {
                            AgentEventPayload::ToolResult {
                                result,
                                is_completion: true,
                                ..
                            } => {
                                completion_output = Some(result.clone());
                            }
                            AgentEventPayload::Stream { text } => {
                                last_stream.push_str(text);
                            }
                            _ => {}
                        }
                        if !is_root && event.agent_name.is_none() {
                            event.agent_name = Some(agent_name.clone());
                        }
                        let h = hub.lock().await;
                        if h.event_sender().try_send(event).is_err() {
                            tracing::debug!(agent = %agent_name, "event dropped (channel full)");
                        }
                    }
                }
            }
            Incoming::Request { id, method, params } => {
                if method == WAIT_AGENT_METHOD {
                    // hub/wait_agent blocks until agent finishes — MUST NOT run
                    // inline or it blocks all subsequent hub/* requests from this agent.
                    info!(agent = %agent_name, %method, "spawning background wait");
                    spawn_wait_agent(hub.clone(), conn.clone(), id, params, agent_name.clone());
                } else if method.starts_with("hub/") {
                    info!(agent = %agent_name, %method, "hub request received");
                    match dispatch_hub_request(&hub, &method, params, agent_name.clone()).await {
                        Ok(result) => {
                            let _ = conn.respond(id, result).await;
                        }
                        Err(e) => {
                            warn!(agent = %agent_name, %method, error = %e, "hub request failed");
                            let _ = conn
                                .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &e)
                                .await;
                        }
                    }
                    info!(agent = %agent_name, %method, "hub request completed");
                } else if method == methods::AGENT_PERMISSION.name
                    || method == methods::AGENT_QUESTION.name
                {
                    info!(agent = %agent_name, %method, "relaying to TUI");
                    relay_to_tui(&hub, &conn, id, &method, params, &agent_name).await;
                    info!(agent = %agent_name, %method, "relay complete");
                } else {
                    warn!(agent = %agent_name, %method, "unknown request");
                    let _ = conn
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
    // Prefer AttemptCompletion output over accumulated stream text
    completion_output.or(if last_stream.is_empty() {
        None
    } else {
        Some(last_stream)
    })
}

/// Spawn hub/wait_agent in a background task so it doesn't block the IO loop.
fn spawn_wait_agent(
    hub: Arc<Mutex<AgentHub>>,
    conn: Arc<Connection>,
    request_id: i64,
    params: serde_json::Value,
    agent_name: String,
) {
    tokio::spawn(async move {
        match crate::dispatch::dispatch_hub_request(
            &hub,
            WAIT_AGENT_METHOD,
            params,
            agent_name.clone(),
        )
        .await
        {
            Ok(result) => {
                let _ = conn.respond(request_id, result).await;
            }
            Err(e) => {
                warn!(agent = %agent_name, "background wait_agent failed: {e}");
                let _ = conn
                    .respond_error(request_id, loopal_ipc::jsonrpc::INVALID_REQUEST, &e)
                    .await;
            }
        }
        info!(agent = %agent_name, "background wait_agent resolved");
    });
}

/// Register agent Connection in Hub and spawn background IO loop.
pub fn start_agent_io(
    hub: Arc<Mutex<AgentHub>>,
    name: &str,
    conn: Arc<Connection>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    is_root: bool,
) {
    // Registration + IO loop in one background task (used by hub_server for TUI/TCP clients)
    let hub2 = hub.clone();
    let n = name.to_string();
    let n2 = name.to_string();
    let conn2 = conn.clone();
    tokio::spawn(async move {
        {
            let mut h = hub.lock().await;
            if let Err(e) = h.register_connection(&n, conn2) {
                tracing::warn!(agent = %n, error = %e, "registration failed");
                return;
            }
        }
        info!(agent = %n, "agent registered in Hub");
        let output = agent_io_loop(hub2, conn, rx, n.clone(), is_root).await;
        let mut h = hub.lock().await;
        // Order matters: emit BEFORE unregister to avoid race with wait_agent.
        // wait_agent checks agent existence → if we unregister first, it returns
        // "not found" and misses the output. By emitting first, any pending watcher
        // gets the output before the agent is removed.
        h.emit_agent_finished(&n2, output);
        h.unregister_connection(&n2);
        info!(agent = %n2, "agent IO loop ended");
    });
}

/// Spawn only the IO loop (registration already done by caller).
pub fn spawn_io_loop(
    hub: Arc<Mutex<AgentHub>>,
    name: &str,
    conn: Arc<Connection>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    is_root: bool,
) {
    let hub2 = hub.clone();
    let n = name.to_string();
    let n2 = name.to_string();
    tokio::spawn(async move {
        let output = agent_io_loop(hub2, conn, rx, n.clone(), is_root).await;
        let mut h = hub.lock().await;
        h.emit_agent_finished(&n2, output);
        h.unregister_connection(&n2);
        info!(agent = %n2, "agent IO loop ended");
    });
}
