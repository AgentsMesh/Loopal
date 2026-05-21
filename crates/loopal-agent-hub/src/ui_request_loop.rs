use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;

use crate::dispatch::dispatch_hub_request_with;
use crate::hub::Hub;

pub(crate) async fn ui_client_io_loop(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    mut rx: mpsc::Receiver<Incoming>,
    name: String,
) {
    info!(client = %name, "UI client IO loop started");
    while let Some(msg) = rx.recv().await {
        let Incoming::Request { id, method, params } = msg else {
            continue;
        };
        let result = if method == methods::VIEW_SNAPSHOT.name {
            crate::view_router::handle_snapshot(&hub, params).await
        } else if method.starts_with("hub/") {
            dispatch_hub_request_with(&dispatcher, &method, params, name.clone()).await
        } else {
            Err(format!(
                "UI clients only support hub/* and view/snapshot, got: {method}"
            ))
        };
        match result {
            Ok(value) => {
                let _ = conn.respond(id, value).await;
            }
            Err(e) => {
                let _ = conn
                    .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &e)
                    .await;
            }
        }
    }
    info!(client = %name, "UI client IO loop ended");
}
