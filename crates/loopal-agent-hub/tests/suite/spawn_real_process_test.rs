use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, agent_io};
use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn managed_spawn_runs_real_child_process_path() {
    let cwd = tempfile::tempdir().unwrap();
    let (events, mut event_rx) = mpsc::channel::<AgentEvent>(64);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let mut hub = Hub::with_cwd(events, cwd.path().into());
    hub.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    let hub = Arc::new(Mutex::new(hub));
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (hub_connection, hub_rx) = Connection::new(hub_transport).into_listening();
    let dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    agent_io::start_agent_io(
        hub.clone(),
        dispatcher,
        loopal_protocol::ROOT_AGENT_NAME,
        hub_connection,
        hub_rx,
        Some(ready_tx),
    );
    ready_rx.await.unwrap();

    let response = tokio::time::timeout(
        Duration::from_secs(20),
        agent.send_request(
            methods::HUB_SPAWN_AGENT.name,
            serde_json::json!({"name": "real-child"}),
        ),
    )
    .await
    .expect("real child spawn timed out")
    .unwrap();

    assert_eq!(response["name"], "real-child");
    assert!(hub.lock().await.registry.agent_info("real-child").is_some());
    let _ = agent
        .send_request(
            methods::HUB_SHUTDOWN_AGENT.name,
            serde_json::json!({"agent": "real-child"}),
        )
        .await;
}
