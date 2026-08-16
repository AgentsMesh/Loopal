use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel(32);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

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

fn request_permission(
    connection: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    id: &'static str,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        connection
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request(id, "Bash", json!({})),
            )
            .await
    })
}

#[tokio::test]
async fn same_name_reconnect_cannot_reuse_old_permission_or_grant() {
    let (hub, raw_rx) = make_hub();
    let _events = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "desktop", loopal_protocol::UiCapabilities::ALL).await;
    let (old_agent, _) = hub_server::connect_local(hub.clone(), "worker");
    wait_registered(&hub, "worker").await;

    let old_pending = request_permission(old_agent.clone(), "old-pending");
    let old = crate::permission_interaction(&hub, "worker", "old-pending").await;
    hub.lock().await.registry.unregister_connection("worker");
    let (new_agent, _) = hub_server::connect_local(hub.clone(), "worker");
    wait_registered(&hub, "worker").await;

    let stale_new = old_agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            crate::permission_request("stale-new", "Bash", json!({})),
        )
        .await;
    assert!(stale_new.is_err());
    assert!(
        !hub.lock()
            .await
            .pending_permissions
            .contains_key(&("worker".into(), "stale-new".into()))
    );

    ui.client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "worker", "tool_call_id": old.id,
                "permission_intent_digest": old.digest,
                "allow": true, "remember_session": true,
            }),
        )
        .await
        .unwrap();
    assert_eq!(old_pending.await.unwrap().unwrap()["allow"], false);

    let current_request = request_permission(new_agent, "current");
    let current = crate::permission_interaction(&hub, "worker", "current").await;
    ui.client
        .respond_permission("worker", &current.id, Some(current.digest), false)
        .await;
    assert_eq!(current_request.await.unwrap().unwrap()["allow"], false);
}
