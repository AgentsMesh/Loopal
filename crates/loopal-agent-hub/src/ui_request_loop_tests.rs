use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::{Dispatcher, DispatcherBuilder, RpcError};
use serde_json::json;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;

use super::{UI_CONTROL_REQUEST_LIMIT, UI_DATA_REQUEST_LIMIT, ui_client_io_loop};
use crate::Hub;

#[path = "ui_request_loop_tests/cancellation.rs"]
mod cancellation;
#[path = "ui_request_loop_tests/concurrency.rs"]
mod concurrency;
#[path = "ui_request_loop_tests/recovery.rs"]
mod recovery;

fn start(dispatcher: Dispatcher) -> (Arc<Connection<Listening>>, JoinHandle<()>) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (server, server_rx) = Connection::new(server_transport).into_listening();
    let hub = Arc::new(Mutex::new(Hub::noop()));
    let task = tokio::spawn(ui_client_io_loop(
        hub,
        Arc::new(dispatcher),
        server,
        server_rx,
        "ui-test".into(),
    ));
    (client, task)
}
