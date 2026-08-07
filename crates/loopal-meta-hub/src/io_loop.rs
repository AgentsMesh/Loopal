//! Concurrent request lifecycle for one authenticated Sub-Hub connection.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
use tokio::task::JoinSet;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;

use crate::dispatch::dispatch_meta_request;
use crate::meta_hub::MetaHub;

/// Ordinary requests admitted concurrently on one Sub-Hub connection.
#[doc(hidden)]
pub const META_DATA_REQUEST_LIMIT: usize = 16;
const META_BUSY_ERROR: i64 = -32000;
const RESPONSE_DEADLINE: Duration = Duration::from_secs(2);

struct InFlightRequest {
    method: String,
    cancel: Option<oneshot::Sender<()>>,
}

struct CompletionGuard {
    id: i64,
    keep_connection: bool,
    done: mpsc::UnboundedSender<(i64, bool)>,
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        let _ = self.done.send((self.id, self.keep_connection));
    }
}

/// Run the authenticated request loop until unregister or disconnect.
pub async fn meta_hub_io_loop(
    meta_hub: Arc<Mutex<MetaHub>>,
    conn: Arc<Connection<Listening>>,
    mut rx: mpsc::Receiver<Incoming>,
    hub_name: String,
) {
    tracing::info!(hub = %hub_name, "Sub-Hub IO loop started");
    let limiter = Arc::new(Semaphore::new(META_DATA_REQUEST_LIMIT));
    let mut requests = JoinSet::new();
    let mut in_flight = HashMap::<i64, InFlightRequest>::new();
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<(i64, bool)>();

    loop {
        tokio::select! {
            biased;
            Some((id, keep_connection)) = done_rx.recv() => {
                in_flight.remove(&id);
                while requests.try_join_next().is_some() {}
                if !keep_connection {
                    break;
                }
            }
            message = rx.recv() => {
                let Some(message) = message else { break };
                match message {
                    Incoming::Notification { method, params }
                        if method == methods::REQUEST_CANCEL.name =>
                    {
                        cancel_request(&mut in_flight, &params);
                    }
                    Incoming::Notification { method, params }
                        if method == methods::META_HEARTBEAT.name =>
                    {
                        let count = params["agent_count"].as_u64().unwrap_or(0) as usize;
                        if meta_hub.lock().await.registry
                            .heartbeat_connection(&hub_name, &conn, count).is_err()
                        {
                            break;
                        }
                    }
                    Incoming::Notification { .. } => {}
                    Incoming::Request { id, method, params } => {
                        if !is_active(&meta_hub, &hub_name, &conn).await {
                            let _ = respond_bounded(
                                &conn,
                                conn.respond_error(
                                    id,
                                    loopal_ipc::jsonrpc::INVALID_REQUEST,
                                    "Sub-Hub registration lease is no longer active",
                                ),
                            ).await;
                            break;
                        }
                        if method == methods::META_UNREGISTER.name {
                            meta_hub.lock().await.registry
                                .unregister_connection(&hub_name, &conn);
                            cancel_all(&mut in_flight);
                            let _ = respond_bounded(
                                &conn,
                                conn.respond(id, serde_json::json!({"ok": true})),
                            ).await;
                            break;
                        }
                        if method == methods::META_HEARTBEAT.name {
                            let count = params["agent_count"].as_u64().unwrap_or(0) as usize;
                            let result = meta_hub.lock().await.registry
                                .heartbeat_connection(&hub_name, &conn, count)
                                .map(|()| serde_json::json!({"ok": true}));
                            if !respond_result(&conn, id, result).await {
                                break;
                            }
                            continue;
                        }
                        if in_flight.contains_key(&id) {
                            if !respond_bounded(
                                &conn,
                                conn.respond_error(
                                    id,
                                    loopal_ipc::jsonrpc::INVALID_REQUEST,
                                    "duplicate in-flight JSON-RPC request id",
                                ),
                            ).await {
                                break;
                            }
                            continue;
                        }
                        let Ok(permit) = limiter.clone().try_acquire_owned() else {
                            if !respond_bounded(
                                &conn,
                                conn.respond_error(
                                    id,
                                    META_BUSY_ERROR,
                                    "too many concurrent MetaHub requests",
                                ),
                            ).await {
                                break;
                            }
                            continue;
                        };
                        let (cancel, cancel_rx) = oneshot::channel();
                        in_flight.insert(id, InFlightRequest {
                            method: method.clone(),
                            cancel: Some(cancel),
                        });
                        let request_hub = meta_hub.clone();
                        let request_conn = conn.clone();
                        let request_name = hub_name.clone();
                        let request_done = done_tx.clone();
                        requests.spawn(async move {
                            let _permit = permit;
                            // The drop guard reports panics/aborts as fatal so
                            // no request id or concurrency slot can leak.
                            let mut completion = CompletionGuard {
                                id,
                                keep_connection: false,
                                done: request_done,
                            };
                            completion.keep_connection = serve_request(
                                request_hub,
                                request_conn,
                                request_name,
                                id,
                                method,
                                params,
                                cancel_rx,
                            ).await;
                        });
                    }
                }
            }
        }
    }

    requests.abort_all();
    while requests.join_next().await.is_some() {}
    meta_hub
        .lock()
        .await
        .registry
        .unregister_connection(&hub_name, &conn);
    close_bounded(&conn).await;
    tracing::info!(hub = %hub_name, "Sub-Hub IO loop ended, cleaned up connection lease");
}

async fn serve_request(
    meta_hub: Arc<Mutex<MetaHub>>,
    conn: Arc<Connection<Listening>>,
    hub_name: String,
    id: i64,
    method: String,
    params: serde_json::Value,
    mut cancel: oneshot::Receiver<()>,
) -> bool {
    let request = dispatch_meta_request(&meta_hub, &method, params, hub_name.clone());
    let result = tokio::select! {
        biased;
        _ = &mut cancel => return true,
        result = request => result,
    };
    // Spawn-handler errors are validation/target-lookup rejections that occur
    // before destination forwarding. Preserve that definitive classification
    // in the successful RPC payload; transport errors remain outcome-unknown.
    let result = if method == methods::META_SPAWN.name {
        result.or_else(|message| {
            Ok(
                loopal_ipc::cross_hub::RemoteSpawnOutcome::RejectedBeforeSideEffect { message }
                    .into_value(),
            )
        })
    } else {
        result
    };
    if let Err(error) = &result {
        tracing::warn!(hub = %hub_name, %method, %error, "request failed");
    }
    respond_result(&conn, id, result).await
}

fn cancel_request(in_flight: &mut HashMap<i64, InFlightRequest>, params: &serde_json::Value) {
    let Some(id) = params.get("id").and_then(serde_json::Value::as_i64) else {
        return;
    };
    let requested_method = params.get("method").and_then(serde_json::Value::as_str);
    if let Some(request) = in_flight.get_mut(&id)
        && requested_method.is_none_or(|method| method == request.method)
        && let Some(cancel) = request.cancel.take()
    {
        let _ = cancel.send(());
    }
}

fn cancel_all(in_flight: &mut HashMap<i64, InFlightRequest>) {
    for request in in_flight.values_mut() {
        if let Some(cancel) = request.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

async fn is_active(
    meta_hub: &Arc<Mutex<MetaHub>>,
    name: &str,
    conn: &Arc<Connection<Listening>>,
) -> bool {
    meta_hub
        .lock()
        .await
        .registry
        .is_active_connection(name, conn)
}

async fn respond_result(
    conn: &Connection<Listening>,
    id: i64,
    result: Result<serde_json::Value, String>,
) -> bool {
    match result {
        Ok(value) => respond_bounded(conn, conn.respond(id, value)).await,
        Err(error) => {
            respond_bounded(
                conn,
                conn.respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &error),
            )
            .await
        }
    }
}

async fn respond_bounded<F>(conn: &Connection<Listening>, response: F) -> bool
where
    F: Future<Output = Result<(), loopal_ipc::RpcError>>,
{
    match tokio::time::timeout(RESPONSE_DEADLINE, response).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(%error, "MetaHub response failed; closing Sub-Hub connection");
            close_bounded(conn).await;
            false
        }
        Err(_) => {
            tracing::warn!("MetaHub response timed out; closing Sub-Hub connection");
            close_bounded(conn).await;
            false
        }
    }
}

async fn close_bounded(conn: &Connection<Listening>) {
    if tokio::time::timeout(RESPONSE_DEADLINE, conn.close())
        .await
        .is_err()
    {
        tracing::warn!("MetaHub Sub-Hub transport close timed out");
    }
}
