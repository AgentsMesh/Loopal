use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::{HandlerCtx, RpcError};
use loopal_protocol::AgentEvent;

use crate::dispatch::dispatch_hub_request_with;
use crate::hub::Hub;
use crate::pending_relay::{handle_agent_permission, handle_agent_question};

const WAIT_AGENT_METHOD: &str = "hub/wait_agent";

/// Run the IO loop for a connected agent. Returns the agent's final output
/// extracted from the `agent/completed` notification — the single authoritative source.
///
/// `dispatcher` is the hub-side request dispatcher. Each spawning site
/// (bootstrap::register_handlers, hub_server::{connect_local, accept_loop})
/// builds its own; build_hub_dispatcher is cheap (~20 register_fn + Arc allocs).
pub async fn agent_io_loop(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    mut rx: tokio::sync::mpsc::Receiver<Incoming>,
    agent_name: String,
) -> Option<String> {
    info!(agent = %agent_name, "agent IO loop started");
    let mut agent_result: Option<String> = None;

    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Notification { method, params } => {
                if method == methods::AGENT_COMPLETED.name {
                    agent_result = params
                        .get("result")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    info!(agent = %agent_name, has_result = agent_result.is_some(), "received agent/completed");
                    break;
                } else if method == methods::AGENT_EVENT.name {
                    forward_agent_event(&hub, &agent_name, params).await;
                }
            }
            Incoming::Request { id, method, params } => {
                handle_request(&hub, &dispatcher, &conn, &agent_name, id, method, params).await;
            }
        }
    }
    agent_result
}

async fn forward_agent_event(hub: &Arc<Mutex<Hub>>, agent_name: &str, params: serde_json::Value) {
    match serde_json::from_value::<AgentEvent>(params) {
        Ok(mut event) => {
            if event.agent_name.is_none() {
                event.agent_name = Some(loopal_protocol::QualifiedAddress::local(
                    agent_name.to_string(),
                ));
            }
            let h = hub.lock().await;
            if h.registry.event_sender().try_send(event).is_err() {
                tracing::warn!(agent = %agent_name, "event dropped (channel full)");
            }
        }
        Err(e) => {
            tracing::warn!(agent = %agent_name, error = %e, "agent/event deserialize failed; dropping");
        }
    }
}

async fn handle_request(
    hub: &Arc<Mutex<Hub>>,
    dispatcher: &Arc<loopal_ipc::Dispatcher>,
    conn: &Arc<Connection<Listening>>,
    agent_name: &str,
    id: i64,
    method: String,
    params: serde_json::Value,
) {
    if method == WAIT_AGENT_METHOD {
        info!(agent = %agent_name, %method, "spawning background wait");
        spawn_wait_agent(
            dispatcher.clone(),
            conn.clone(),
            id,
            params,
            agent_name.to_string(),
        );
        return;
    }
    if method.starts_with("hub/") || method.starts_with("meta/") {
        info!(agent = %agent_name, %method, "hub request received");
        let ctx = HandlerCtx::new(agent_name.to_string());
        match dispatcher.dispatch(&method, params, &ctx).await {
            Ok(result) => {
                let _ = conn.respond(id, result).await;
            }
            Err(e) => {
                let msg = rpc_error_message(&e);
                warn!(agent = %agent_name, %method, error = %msg, "hub request failed");
                let _ = conn
                    .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &msg)
                    .await;
            }
        }
        info!(agent = %agent_name, %method, "hub request completed");
        return;
    }
    if method == methods::AGENT_PERMISSION.name {
        handle_agent_permission(hub, conn.clone(), id, params, agent_name).await;
        return;
    }
    if method == methods::AGENT_QUESTION.name {
        handle_agent_question(hub, conn.clone(), id, params, agent_name).await;
        return;
    }
    warn!(agent = %agent_name, %method, "unknown request");
    let _ = conn
        .respond_error(
            id,
            loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
            &format!("unknown: {method}"),
        )
        .await;
}

fn rpc_error_message(e: &RpcError) -> String {
    match e {
        RpcError::Remote { message, .. } => message.clone(),
        other => other.to_string(),
    }
}

fn spawn_wait_agent(
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    request_id: i64,
    params: serde_json::Value,
    agent_name: String,
) {
    tokio::spawn(async move {
        match dispatch_hub_request_with(
            dispatcher.as_ref(),
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
