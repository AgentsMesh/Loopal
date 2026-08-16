use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, UiSession, hub_server};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, UiCapabilities};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

async fn wait_registered(hub: &Arc<Mutex<Hub>>, name: &str) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while hub
            .lock()
            .await
            .registry
            .get_agent_connection(name)
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("agent registration timed out");
}

#[tokio::test]
async fn closed_authoritative_queue_removes_pending_and_denies() {
    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(1);
    drop(event_rx);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let _ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _incoming) = hub_server::connect_local(hub.clone(), "main");
    wait_registered(&hub, "main").await;

    let response = tokio::time::timeout(
        Duration::from_secs(2),
        agent.send_request(
            methods::AGENT_PERMISSION.name,
            crate::permission_request("closed", "Bash", json!({})),
        ),
    )
    .await
    .expect("permission response timed out")
    .unwrap();

    assert_eq!(response["allow"], false);
    assert!(hub.lock().await.pending_permissions.is_empty());
}

#[tokio::test(start_paused = true)]
async fn admitted_permission_timeout_denies_and_removes_pending() {
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    hub.lock()
        .await
        .set_pending_interaction_timeout(Duration::from_secs(5));
    let _ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _incoming) = hub_server::connect_local(hub.clone(), "main");
    wait_registered(&hub, "main").await;

    let request = tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request("timeout", "Bash", json!({})),
            )
            .await
    });
    crate::permission_interaction(&hub, "main", "timeout").await;
    event_rx.recv().await.expect("permission event admitted");
    tokio::time::advance(Duration::from_secs(5)).await;
    tokio::task::yield_now().await;

    let response = request.await.unwrap().unwrap();
    assert_eq!(response["allow"], false);
    assert!(hub.lock().await.pending_permissions.is_empty());
}
