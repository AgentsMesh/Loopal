use std::sync::Arc;

use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::Connection;
use loopal_protocol::AgentEvent;
use tokio::sync::{Mutex, mpsc};

fn hub() -> Arc<Mutex<Hub>> {
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(8);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    Arc::new(Mutex::new(Hub::new(event_tx)))
}

async fn register_with_parent(hub: Arc<Mutex<Hub>>, parent: &str) -> Result<String, String> {
    let (_client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (server, server_rx) = Connection::new(server_transport).into_listening();
    register_agent_connection(hub, "child", server, server_rx, Some(parent), None, None).await
}

#[tokio::test]
async fn remote_parent_is_rejected_before_registration() {
    let hub = hub();

    let error = register_with_parent(hub.clone(), "other-hub/parent")
        .await
        .unwrap_err();

    assert_eq!(error, "local child registration requires a local parent");
    assert!(hub.lock().await.registry.agent_info("child").is_none());
}

#[tokio::test]
async fn missing_local_parent_is_rejected_before_registration() {
    let hub = hub();

    let error = register_with_parent(hub.clone(), "missing")
        .await
        .unwrap_err();

    assert_eq!(error, "parent agent 'missing' is not active");
    assert!(hub.lock().await.registry.agent_info("child").is_none());
}
