use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::{HandlerCtx, RpcError};
use loopal_protocol::{AgentCompletion, AgentEvent, PROTOCOL_ERROR_REASON, TRANSPORT_ERROR_REASON};

use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::dispatch::dispatch_hub_request_with;
use crate::hub::Hub;
use crate::pending_relay::{
    handle_agent_permission, handle_agent_plan_approval, handle_agent_question,
};

const WAIT_AGENT_METHOD: &str = "hub/wait_agent";

/// Run the IO loop for a connected agent. Returns the authoritative typed
/// `agent/completed` payload. Closing without one is a transport failure.
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
) -> AgentCompletion {
    info!(agent = %agent_name, "agent IO loop started");

    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Notification { method, params } => {
                if method == methods::AGENT_COMPLETED.name {
                    match serde_json::from_value::<AgentCompletion>(params) {
                        Ok(completion) => {
                            info!(
                                agent = %agent_name,
                                reason = %completion.reason,
                                has_result = completion.result.is_some(),
                                "received agent/completed"
                            );
                            return completion;
                        }
                        Err(error) => {
                            warn!(
                                agent = %agent_name,
                                %error,
                                "malformed agent/completed"
                            );
                            return AgentCompletion::new(
                                PROTOCOL_ERROR_REASON,
                                Some(format!("malformed agent/completed: {error}")),
                            );
                        }
                    }
                } else if method == methods::AGENT_EVENT.name {
                    if let Err(error) = forward_agent_event(&hub, &conn, &agent_name, params).await
                    {
                        warn!(agent = %agent_name, %error, "authoritative event transport failed");
                        return AgentCompletion::new(TRANSPORT_ERROR_REASON, Some(error));
                    }
                } else if method == methods::REQUEST_CANCEL.name {
                    let Some(request_id) = params.get("id").and_then(|value| value.as_i64()) else {
                        warn!(agent = %agent_name, "malformed request cancellation ignored");
                        continue;
                    };
                    crate::pending_relay::cancel_pending_request(
                        &hub,
                        &agent_name,
                        &conn,
                        request_id,
                    )
                    .await;
                }
            }
            Incoming::Request { id, method, params } => {
                handle_request(&hub, &dispatcher, &conn, &agent_name, id, method, params).await;
            }
        }
    }
    AgentCompletion::new(
        TRANSPORT_ERROR_REASON,
        Some("agent transport closed before agent/completed".into()),
    )
}

async fn forward_agent_event(
    hub: &Arc<Mutex<Hub>>,
    connection: &Arc<Connection<Listening>>,
    agent_name: &str,
    params: serde_json::Value,
) -> Result<(), String> {
    match serde_json::from_value::<AgentEvent>(params) {
        Ok(mut event) => {
            if event.agent_name.is_none() {
                event.agent_name = Some(loopal_protocol::QualifiedAddress::local(
                    agent_name.to_string(),
                ));
            }
            let source_matches = event
                .agent_name
                .as_ref()
                .is_some_and(|address| address.is_local() && address.agent == agent_name);
            if !source_matches {
                warn!(agent = %agent_name, claimed = ?event.agent_name, "agent/event source mismatch; dropping");
                return Ok(());
            }
            let mut delivery = {
                let mut h = hub.lock().await;
                let Some(event) = h
                    .registry
                    .prepare_connection_event(agent_name, connection, event)
                else {
                    tracing::debug!(agent = %agent_name, "stale generation event dropped");
                    return Ok(());
                };
                PreparedAuthoritativeEvent::from_hub(&h, event)
            };
            // Agent events are the authoritative ordered state stream. In
            // particular, dropping AwaitingInput or ToolResult leaves every
            // downstream UI permanently active. Apply bounded backpressure
            // after releasing the Hub lock so the reducer can drain the queue.
            delivery
                .deliver()
                .await
                .map_err(|error| format!("agent '{agent_name}' event delivery failed: {error}"))?;
        }
        Err(e) => {
            tracing::warn!(agent = %agent_name, error = %e, "agent/event deserialize failed; dropping");
        }
    }
    Ok(())
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
    if method == methods::AGENT_PLAN_APPROVAL.name {
        handle_agent_plan_approval(hub, conn.clone(), id, params, agent_name).await;
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use loopal_ipc::connection::Connection;
    use loopal_ipc::duplex_pair;
    use loopal_protocol::{AgentEvent, AgentEventPayload};
    use tokio::sync::{Mutex, mpsc};

    use super::forward_agent_event;
    use crate::Hub;

    #[tokio::test]
    async fn full_event_queue_backpressures_without_dropping_terminal_state_or_holding_hub_lock() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Running))
            .await
            .unwrap();
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let (transport, _peer) = duplex_pair();
        let (connection, _incoming) = Connection::new(transport).into_listening();
        hub.lock()
            .await
            .registry
            .register_connection("main", connection.clone())
            .unwrap();

        let delivery = tokio::spawn({
            let hub = hub.clone();
            let connection = connection.clone();
            async move {
                forward_agent_event(
                    &hub,
                    &connection,
                    "main",
                    serde_json::to_value(AgentEvent::root(AgentEventPayload::AwaitingInput))
                        .unwrap(),
                )
                .await
                .unwrap();
            }
        });
        tokio::task::yield_now().await;
        assert!(
            !delivery.is_finished(),
            "a full queue must backpressure rather than drop the terminal event"
        );
        let hub_guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
            .await
            .expect("backpressured delivery must release the Hub lock");
        drop(hub_guard);

        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Running
        ));
        tokio::time::timeout(Duration::from_millis(100), delivery)
            .await
            .expect("delivery should resume when queue capacity is available")
            .unwrap();
        let delivered = event_rx.recv().await.unwrap();
        assert!(matches!(
            delivered.payload,
            AgentEventPayload::AwaitingInput
        ));
        assert!(delivered.routing_generation.is_some());
    }
}
