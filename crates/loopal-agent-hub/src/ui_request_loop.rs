use std::sync::Arc;

use tokio::sync::{Mutex, Semaphore, mpsc};
use tokio::task::JoinSet;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;

use crate::dispatch::dispatch_hub_request_with;
use crate::hub::Hub;

#[path = "ui_request_policy.rs"]
mod policy;
use policy::{is_control_request, is_ui_request};

pub(crate) const UI_DATA_REQUEST_LIMIT: usize = 16;
pub(crate) const UI_CONTROL_REQUEST_LIMIT: usize = 4;
const UI_BUSY_ERROR: i64 = -32000;
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
    let mut requests = JoinSet::new();
    while let Some(msg) = rx.recv().await {
        let Incoming::Request { id, method, params } = msg else {
            continue;
        };
        while requests.try_join_next().is_some() {}
        let limiter = if is_control_request(&method) {
            control_limiter.clone()
        } else {
            data_limiter.clone()
        };
        let Ok(permit) = limiter.try_acquire_owned() else {
            let _ = conn
                .respond_error(id, UI_BUSY_ERROR, "too many concurrent UI requests")
                .await;
            continue;
        };
        let request_hub = hub.clone();
        let request_dispatcher = dispatcher.clone();
        let request_conn = conn.clone();
        let request_name = name.clone();
        requests.spawn(async move {
            let _permit = permit;
            serve_request(
                request_hub,
                request_dispatcher,
                request_conn,
                request_name,
                id,
                method,
                params,
            )
            .await;
        });
    }
    requests.abort_all();
    while requests.join_next().await.is_some() {}
    info!(client = %name, "UI client IO loop ended");
}
async fn serve_request(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    name: String,
    id: i64,
    method: String,
    params: serde_json::Value,
) {
    let result = if method == methods::VIEW_SNAPSHOT.name {
        crate::view_router::handle_snapshot(&hub, params).await
    } else if is_ui_request(&method) {
        dispatch_hub_request_with(&dispatcher, &method, params, name).await
    } else {
        Err(format!("method is not allowed for UI clients: {method}"))
    };
    match result {
        Ok(value) => {
            let _ = conn.respond(id, value).await;
        }
        Err(error) => {
            let _ = conn
                .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &error)
                .await;
        }
    }
}

#[cfg(test)]
#[path = "ui_request_loop_tests.rs"]
mod concurrency_tests;
