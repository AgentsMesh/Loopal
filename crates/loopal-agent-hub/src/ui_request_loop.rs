use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
use tokio::task::JoinSet;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;

use crate::dispatch::dispatch_hub_request_with;
use crate::hub::Hub;

#[path = "ui_request_policy.rs"]
mod policy;
use policy::{is_control_request, is_recovery_request, is_ui_request};

pub(crate) const UI_DATA_REQUEST_LIMIT: usize = 16;
pub(crate) const UI_CONTROL_REQUEST_LIMIT: usize = 4;
pub(crate) const UI_RECOVERY_REQUEST_LIMIT: usize = 2;
const UI_BUSY_ERROR: i64 = -32000;
#[cfg(not(test))]
const UI_RESPONSE_DEADLINE: Duration = Duration::from_secs(2);
#[cfg(test)]
const UI_RESPONSE_DEADLINE: Duration = Duration::from_millis(100);

struct InFlightRequest {
    method: String,
    cancel: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
struct RequestContext {
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    name: String,
}

struct CompletionGuard {
    id: i64,
    done: mpsc::UnboundedSender<(i64, bool)>,
    completed: bool,
}

impl CompletionGuard {
    fn new(id: i64, done: mpsc::UnboundedSender<(i64, bool)>) -> Self {
        Self {
            id,
            done,
            completed: false,
        }
    }

    fn complete(mut self, keep_connection: bool) {
        self.completed = true;
        let _ = self.done.send((self.id, keep_connection));
    }
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.done.send((self.id, false));
        }
    }
}

pub(crate) async fn ui_client_io_loop(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    mut rx: mpsc::Receiver<Incoming>,
    name: String,
) {
    info!(client = %name, "UI client IO loop started");
    let data_limiter = Arc::new(Semaphore::new(UI_DATA_REQUEST_LIMIT));
    let control_limiter = Arc::new(Semaphore::new(UI_CONTROL_REQUEST_LIMIT));
    let recovery_limiter = Arc::new(Semaphore::new(UI_RECOVERY_REQUEST_LIMIT));
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
                    close_response_transport(&conn).await;
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
                    Incoming::Notification { .. } => {}
                    Incoming::Request { id, method, params } => {
                        while requests.try_join_next().is_some() {}
                        if in_flight.contains_key(&id) {
                            if !respond_duplicate(&conn, id).await {
                                break;
                            }
                            continue;
                        }
                        let limiter = if is_recovery_request(&method) {
                            recovery_limiter.clone()
                        } else if is_control_request(&method) {
                            control_limiter.clone()
                        } else {
                            data_limiter.clone()
                        };
                        let Ok(permit) = limiter.try_acquire_owned() else {
                            if !respond_busy(&conn, id).await {
                                break;
                            }
                            continue;
                        };
                        let (cancel, cancel_rx) = oneshot::channel();
                        in_flight.insert(id, InFlightRequest {
                            method: method.clone(),
                            cancel: Some(cancel),
                        });
                        let context = RequestContext {
                            hub: hub.clone(),
                            dispatcher: dispatcher.clone(),
                            conn: conn.clone(),
                            name: name.clone(),
                        };
                        let completion = CompletionGuard::new(id, done_tx.clone());
                        requests.spawn(async move {
                            let _permit = permit;
                            let keep_connection =
                                serve_request(context, id, method, params, cancel_rx).await;
                            completion.complete(keep_connection);
                        });
                    }
                }
            }
        }
    }
    requests.abort_all();
    while requests.join_next().await.is_some() {}
    info!(client = %name, "UI client IO loop ended");
}
async fn serve_request(
    context: RequestContext,
    id: i64,
    method: String,
    params: serde_json::Value,
    mut cancel: oneshot::Receiver<()>,
) -> bool {
    let request = async {
        if method == methods::VIEW_SNAPSHOT.name {
            crate::view_router::handle_snapshot(&context.hub, params).await
        } else if is_ui_request(&method) {
            dispatch_hub_request_with(&context.dispatcher, &method, params, context.name).await
        } else {
            Err(format!("method is not allowed for UI clients: {method}"))
        }
    };
    let result = tokio::select! {
        biased;
        _ = &mut cancel => return true,
        result = request => result,
    };
    respond_result(&context.conn, id, result).await
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

async fn respond_busy(conn: &Connection<Listening>, id: i64) -> bool {
    respond_bounded(
        conn,
        conn.respond_error(id, UI_BUSY_ERROR, "too many concurrent UI requests"),
    )
    .await
}

async fn respond_duplicate(conn: &Connection<Listening>, id: i64) -> bool {
    respond_bounded(
        conn,
        conn.respond_error(
            id,
            loopal_ipc::jsonrpc::INVALID_REQUEST,
            "duplicate in-flight JSON-RPC request id",
        ),
    )
    .await
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
    F: std::future::Future<Output = Result<(), loopal_ipc::RpcError>>,
{
    match tokio::time::timeout(UI_RESPONSE_DEADLINE, response).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(%error, "UI response failed; closing connection");
            close_response_transport(conn).await;
            false
        }
        Err(_) => {
            tracing::warn!("UI response timed out; closing connection");
            close_response_transport(conn).await;
            false
        }
    }
}

async fn close_response_transport(conn: &Connection<Listening>) {
    if tokio::time::timeout(UI_RESPONSE_DEADLINE, conn.close())
        .await
        .is_err()
    {
        tracing::warn!("UI transport close timed out after response failure");
    }
}

#[cfg(test)]
#[path = "ui_request_loop_tests.rs"]
mod concurrency_tests;
