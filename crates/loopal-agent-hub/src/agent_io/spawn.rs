use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::Envelope;

use crate::finish::finish_and_deliver;
use crate::hub::Hub;

use super::dispatch_loop::agent_io_loop;

struct RegisteredAgent {
    completion_rx: tokio::sync::mpsc::Receiver<Envelope>,
    root_services: Option<(
        Arc<crate::spawn_registry::SpawnRegistry>,
        Arc<crate::HubMcpService>,
        std::path::PathBuf,
    )>,
}

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
    let n = name.to_string();
    tokio::spawn(async move {
        let registered = match register_agent(&hub, &n, conn.clone(), false).await {
            Ok(registered) => registered,
            Err(error) => {
                tracing::warn!(agent = %n, %error, "registration failed");
                drop(ready_tx);
                close_bounded(&conn).await;
                return;
            }
        };
        run_registered_agent(hub, dispatcher, n, conn, rx, registered, ready_tx).await;
    });
}

/// Activate an ACKed TCP registration reservation and start its IO owner.
///
/// The caller must reserve the same `(name, conn)` before writing the ACK.
pub(crate) async fn start_reserved_agent_io(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: String,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
) -> Result<(), String> {
    let registered = register_agent(&hub, &name, conn.clone(), true).await?;
    tokio::spawn(run_registered_agent(
        hub, dispatcher, name, conn, rx, registered, None,
    ));
    Ok(())
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
        let completion = agent_io_loop(hub2, dispatcher, conn.clone(), rx, n.clone()).await;
        finish_and_deliver(&hub, &n2, completion, &conn).await;
        info!(agent = %n2, "agent IO loop ended");
    });
}

async fn register_agent(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    reserved: bool,
) -> Result<RegisteredAgent, String> {
    let (completion_tx, completion_rx) = tokio::sync::mpsc::channel::<Envelope>(32);
    let root_services = {
        let mut hub = hub.lock().await;
        if reserved {
            hub.registry
                .activate_reserved_connection(name, conn, completion_tx)?;
        } else {
            hub.registry.register_connection_with_parent(
                name,
                conn,
                None,
                None,
                Some(completion_tx),
            )?;
        }
        (name == loopal_protocol::ROOT_AGENT_NAME).then(|| {
            (
                hub.spawn_registry.clone(),
                hub.mcp_service.clone(),
                hub.default_cwd.clone(),
            )
        })
    };
    Ok(RegisteredAgent {
        completion_rx,
        root_services,
    })
}

async fn run_registered_agent(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: String,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    registered: RegisteredAgent,
    ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
) {
    if let Some((registry, mcp, cwd)) = registered.root_services {
        registry.register(name.clone(), cwd.clone(), None);
        mcp.on_agent_attach(name.clone(), cwd, None).await;
    }
    crate::spawn_manager::spawn_completion_bridge(&name, conn.clone(), registered.completion_rx);
    info!(agent = %name, "agent registered in Hub");
    if let Some(tx) = ready_tx {
        let _ = tx.send(());
    }
    let completion = agent_io_loop(hub.clone(), dispatcher, conn.clone(), rx, name.clone()).await;
    finish_and_deliver(&hub, &name, completion, &conn).await;
    info!(agent = %name, "agent IO loop ended");
}

async fn close_bounded(conn: &Connection<Listening>) {
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), conn.close()).await;
}
