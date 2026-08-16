use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, QualifiedAddress};
use tokio::sync::{Mutex, mpsc};

use super::{ProcessFuture, SpawnProcess, initialize_and_register};
use crate::Hub;
use crate::spawn_manager::{PreparedSpawn, SpawnRequestLease};
use crate::types::SpawnAuthority;

struct FakeProcess {
    transport: Arc<dyn Transport>,
    shutdown: Arc<AtomicBool>,
}

impl SpawnProcess for FakeProcess {
    fn transport(&self) -> Arc<dyn Transport> {
        self.transport.clone()
    }

    fn shutdown(self) -> ProcessFuture {
        self.shutdown.store(true, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }

    fn wait(self) -> ProcessFuture {
        Box::pin(async { Ok(()) })
    }
}

async fn harness(
    initialize_ok: bool,
    start: impl Fn(serde_json::Value) -> Result<serde_json::Value, &'static str> + Send + 'static,
) -> (Arc<Mutex<Hub>>, PreparedSpawn, FakeProcess, Arc<AtomicBool>) {
    let (events, mut event_rx) = mpsc::channel::<AgentEvent>(16);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (_parent_peer, parent_transport) = loopal_ipc::duplex_pair();
    let parent = Connection::new(parent_transport).into_listening().0;
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("parent", parent, None, None, None)
        .unwrap();
    let prepared = PreparedSpawn {
        name: "child".into(),
        request_lease: SpawnRequestLease::Agent(execution.clone()),
        cwd: PathBuf::from("/tmp"),
        prompt: None,
        parent: Some(QualifiedAddress::local("parent")),
        parent_execution: Some(execution),
        authority: SpawnAuthority::default(),
        agent_type: None,
        depth: 1,
        fork_context: None,
        workflow_permission_causation: None,
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        notify_parent_on_completion: true,
        root_cwd: PathBuf::from("/tmp"),
        root: "parent".into(),
    };
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (server, mut incoming) = Connection::new(server_transport).into_listening();
    tokio::spawn(async move {
        while let Some(Incoming::Request { id, method, params }) = incoming.recv().await {
            if method == methods::INITIALIZE.name {
                if initialize_ok {
                    server
                        .respond(id, serde_json::json!({"protocol_version": 1}))
                        .await
                        .unwrap();
                } else {
                    server
                        .respond_error(id, -32603, "init rejected")
                        .await
                        .unwrap();
                }
            } else if method == methods::AGENT_START.name {
                match start(params) {
                    Ok(value) => server.respond(id, value).await.unwrap(),
                    Err(error) => server.respond_error(id, -32603, error).await.unwrap(),
                }
            }
        }
    });
    let shutdown = Arc::new(AtomicBool::new(false));
    let process = FakeProcess {
        transport: client_transport,
        shutdown: shutdown.clone(),
    };
    (hub, prepared, process, shutdown)
}

#[tokio::test]
async fn start_rpc_failure_terminalizes_and_shuts_down_child() {
    let (hub, prepared, process, shutdown) = harness(true, |_| Err("start rejected")).await;
    let error = initialize_and_register(hub.clone(), prepared, process)
        .await
        .unwrap_err();
    assert!(error.contains("start rejected"));
    assert!(shutdown.load(Ordering::SeqCst));
    assert!(hub.lock().await.registry.agent_info("child").is_some());
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("child")
            .is_none()
    );
}

#[tokio::test]
async fn initialize_failure_shuts_down_orphan() {
    let (hub, prepared, process, shutdown) =
        harness(false, |_| unreachable!("start must not run")).await;
    let error = initialize_and_register(hub, prepared, process)
        .await
        .unwrap_err();
    assert!(error.contains("agent initialize failed"));
    assert!(shutdown.load(Ordering::SeqCst));
}

#[tokio::test]
async fn registration_failure_shuts_down_initialized_orphan() {
    let (hub, prepared, process, shutdown) = harness(true, |params| {
        Ok(serde_json::json!({"session_id": params["session_id"]}))
    })
    .await;
    hub.lock().await.max_total_agents = 0;
    let error = initialize_and_register(hub, prepared, process)
        .await
        .unwrap_err();
    assert!(error.contains("Spawn budget exhausted"));
    assert!(shutdown.load(Ordering::SeqCst));
}

#[tokio::test]
async fn changed_session_id_is_rejected_and_child_is_shutdown() {
    let (_hub, prepared, process, shutdown) =
        harness(true, |_| Ok(serde_json::json!({"session_id": "wrong"}))).await;
    let error = initialize_and_register(_hub, prepared, process)
        .await
        .unwrap_err();
    assert!(error.contains("different Hub-issued session id"));
    assert!(shutdown.load(Ordering::SeqCst));
}
