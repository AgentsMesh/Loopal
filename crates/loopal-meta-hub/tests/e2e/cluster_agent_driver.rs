use std::sync::Arc;

use loopal_agent_hub::Hub;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use serde_json::Value;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

pub struct AgentDriver {
    connection: Arc<Connection<Listening>>,
    drain: JoinHandle<()>,
}

impl AgentDriver {
    pub async fn connect(hub: Arc<Mutex<Hub>>, name: &str) -> Self {
        let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
        let (connection, incoming) = Connection::new(agent_transport).into_listening();
        let (hub_connection, hub_incoming) = Connection::new(hub_transport).into_listening();
        let dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
            hub.clone(),
        ));
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        loopal_agent_hub::agent_io::start_agent_io(
            hub,
            dispatcher,
            name,
            hub_connection,
            hub_incoming,
            Some(ready_tx),
        );
        ready_rx.await.expect("agent driver registration failed");
        Self {
            connection: connection.clone(),
            drain: drain_reverse_requests(connection, incoming),
        }
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.connection
            .send_request(method, params)
            .await
            .map_err(|error| error.to_string())
    }
}

impl Drop for AgentDriver {
    fn drop(&mut self) {
        self.drain.abort();
    }
}

fn drain_reverse_requests(
    connection: Arc<Connection<Listening>>,
    mut incoming: mpsc::Receiver<Incoming>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = incoming.recv().await {
            if let Incoming::Request { id, .. } = message {
                let _ = connection
                    .respond(id, serde_json::json!({"ok": true}))
                    .await;
            }
        }
    })
}
