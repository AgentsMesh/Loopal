//! Integration tests for Hub agent lifecycle and routing.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::hub_server;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::rpc_error::RpcError;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Spawn a mock agent that auto-responds to all requests with {"ok": true}.
fn spawn_mock_agent(conn: Arc<Connection<Listening>>, mut rx: mpsc::Receiver<Incoming>) {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = conn.respond(id, json!({"ok": true})).await;
            }
        }
    });
}

include!("hub_integration_test/registration_routing.rs");
include!("hub_integration_test/control_errors_events.rs");
