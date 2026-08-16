use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::Envelope;
use tokio::sync::Mutex;
use tracing::info;

use crate::finish::finish_and_deliver_exact;
use crate::hub::Hub;
use crate::types::{AgentExecutionRef, AgentOrigin, AgentRuntimeFacts, SpawnAuthority};

use super::agent_io_loop_exact;

struct RegisteredAgent {
    execution: AgentExecutionRef,
    completion_rx: tokio::sync::mpsc::Receiver<Envelope>,
    root_services: Option<(Arc<crate::HubMcpService>, std::path::PathBuf)>,
}

pub fn start_agent_io(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
) {
    start_agent_io_with_origin(
        hub,
        dispatcher,
        name,
        conn,
        rx,
        ready_tx,
        AgentOrigin::ManagedRoot,
    );
}

fn start_agent_io_with_origin(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
    origin: AgentOrigin,
) {
    let name = name.to_string();
    tokio::spawn(async move {
        let registered = match register_agent(&hub, &name, conn.clone(), false, origin).await {
            Ok(registered) => registered,
            Err(error) => {
                tracing::warn!(agent = %name, %error, "registration failed");
                drop(ready_tx);
                close_bounded(&conn).await;
                return;
            }
        };
        run_registered_agent(hub, dispatcher, name, conn, rx, registered, ready_tx).await;
    });
}

pub(crate) async fn start_reserved_agent_io(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: String,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
) -> Result<(), String> {
    let registered =
        register_agent(&hub, &name, conn.clone(), true, AgentOrigin::ExternalTcp).await?;
    tokio::spawn(run_registered_agent(
        hub, dispatcher, name, conn, rx, registered, None,
    ));
    Ok(())
}

async fn register_agent(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    reserved: bool,
    origin: AgentOrigin,
) -> Result<RegisteredAgent, String> {
    let (completion_tx, completion_rx) = tokio::sync::mpsc::channel::<Envelope>(32);
    let (execution, root_services) = {
        let mut hub = hub.lock().await;
        let execution = if reserved {
            hub.registry
                .activate_reserved_connection_with_execution(name, conn, completion_tx)?
        } else {
            hub.registry.register_connection_with_parent_execution(
                name,
                conn,
                None,
                None,
                Some(completion_tx),
            )?
        };
        let cwd = hub.default_cwd.clone();
        let facts = match origin {
            AgentOrigin::ManagedRoot => {
                AgentRuntimeFacts::root(cwd.clone(), hub.root_spawn_authority())
            }
            AgentOrigin::ExternalTcp => AgentRuntimeFacts {
                origin,
                root_cwd: cwd.clone(),
                cwd: cwd.clone(),
                root: name.to_string(),
                parent: None,
                depth: 0,
                session_id: None,
                workflow_permission_causation: None,
                workflow_attempt_capability_digest: None,
                workflow_completion_result_limit: None,
                spawn: SpawnAuthority::default(),
            },
            AgentOrigin::ManagedChild => unreachable!("child registration uses spawn admission"),
        };
        if !hub.registry.set_runtime_facts(&execution, facts) {
            hub.registry.unregister_exact(&execution);
            return Err("Agent lease changed before runtime facts were installed".into());
        }
        if origin == AgentOrigin::ManagedRoot {
            hub.spawn_registry
                .register_exact(execution.clone(), cwd.clone(), None);
        }
        let services = (origin == AgentOrigin::ManagedRoot
            && name == loopal_protocol::ROOT_AGENT_NAME)
            .then(|| (hub.mcp_service.clone(), cwd));
        (execution, services)
    };
    Ok(RegisteredAgent {
        execution,
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
    if let Some((mcp, cwd)) = registered.root_services {
        mcp.on_agent_attach(registered.execution.clone(), cwd).await;
    }
    crate::spawn_manager::spawn_completion_bridge(&name, conn.clone(), registered.completion_rx);
    info!(agent = %name, "agent registered in Hub");
    if let Some(tx) = ready_tx {
        let _ = tx.send(());
    }
    let completion = agent_io_loop_exact(
        hub.clone(),
        dispatcher,
        conn.clone(),
        rx,
        name.clone(),
        registered.execution.clone(),
    )
    .await;
    finish_and_deliver_exact(&hub, &name, completion, &conn, &registered.execution).await;
    info!(agent = %name, "agent IO loop ended");
}

async fn close_bounded(conn: &Connection<Listening>) {
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), conn.close()).await;
}
