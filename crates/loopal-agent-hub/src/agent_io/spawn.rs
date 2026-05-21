use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::Envelope;

use crate::finish::finish_and_deliver;
use crate::hub::Hub;

use super::dispatch_loop::agent_io_loop;

/// Register the agent connection with the Hub and spawn the IO loop.
///
/// `ready_tx`, if provided, is signaled once the agent is registered and the
/// loop is about to start consuming `rx` — i.e. it is now safe to send an
/// outbound request that may trigger a reverse `hub/*` call.
pub fn start_agent_io(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
) {
    let hub2 = hub.clone();
    let n = name.to_string();
    let n2 = name.to_string();
    let conn2 = conn.clone();
    let conn3 = conn.clone();
    tokio::spawn(async move {
        let (completion_tx, completion_rx) = tokio::sync::mpsc::channel::<Envelope>(32);
        {
            let mut h = hub.lock().await;
            if let Err(e) = h.registry.register_connection_with_parent(
                &n,
                conn2,
                None,
                None,
                Some(completion_tx),
            ) {
                tracing::warn!(agent = %n, error = %e, "registration failed");
                if let Some(tx) = ready_tx {
                    drop(tx);
                }
                return;
            }
        }
        crate::spawn_manager::spawn_completion_bridge(&n, conn3, completion_rx);
        info!(agent = %n, "agent registered in Hub");
        if let Some(tx) = ready_tx {
            let _ = tx.send(());
        }
        let output = agent_io_loop(hub2, dispatcher, conn.clone(), rx, n.clone()).await;
        finish_and_deliver(&hub, &n2, output, &conn).await;
        info!(agent = %n2, "agent IO loop ended");
    });
}

/// Spawn ONLY the IO loop. The caller is responsible for having already
/// registered the agent with the Hub.
pub fn spawn_io_loop(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
) {
    let hub2 = hub.clone();
    let n = name.to_string();
    let n2 = name.to_string();
    tokio::spawn(async move {
        let output = agent_io_loop(hub2, dispatcher, conn.clone(), rx, n.clone()).await;
        finish_and_deliver(&hub, &n2, output, &conn).await;
        info!(agent = %n2, "agent IO loop ended");
    });
}
