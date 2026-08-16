use std::sync::Arc;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, UiCapabilities};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

fn request_permission(
    agent: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    id: &'static str,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request(id, "Write", json!({})),
            )
            .await
    })
}

async fn setup() -> (
    Arc<Mutex<Hub>>,
    Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
) {
    let (events, event_rx) = mpsc::channel::<AgentEvent>(16);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let _event_loop = start_event_loop(hub.clone(), event_rx);
    let (agent, _) = hub_server::connect_local(hub.clone(), "main");
    (hub, agent)
}

#[tokio::test]
async fn incapable_ui_cannot_consume_pending_permission() {
    let (hub, agent) = setup().await;
    let capable = UiSession::connect(hub.clone(), "capable", UiCapabilities::ALL).await;
    let observer = UiSession::connect(hub.clone(), "observer", UiCapabilities::NONE).await;
    let request = request_permission(agent, "incapable");
    let interaction = crate::permission_interaction(&hub, "main", "incapable").await;

    let unauthorized = observer
        .client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "main",
                "tool_call_id": interaction.id,
                "permission_intent_digest": interaction.digest,
                "allow": true,
            }),
        )
        .await;
    assert!(matches!(
        unauthorized,
        Err(loopal_ipc::RpcError::Remote { .. })
    ));
    assert_eq!(hub.lock().await.pending_permissions.len(), 1);

    capable
        .client
        .respond_permission("main", &interaction.id, Some(interaction.digest), false)
        .await;
    assert_eq!(request.await.unwrap().unwrap()["allow"], false);
}

#[tokio::test]
async fn revoked_permission_lease_cannot_consume_pending() {
    let (hub, agent) = setup().await;
    let revoked = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let current = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let request = request_permission(agent, "revoked");
    let interaction = crate::permission_interaction(&hub, "main", "revoked").await;
    hub.lock().await.ui.unregister_client(&revoked.lease_id);

    let stale = revoked
        .client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "main",
                "tool_call_id": interaction.id,
                "permission_intent_digest": interaction.digest,
                "allow": true,
            }),
        )
        .await;
    assert!(matches!(stale, Err(loopal_ipc::RpcError::Remote { .. })));
    assert_eq!(hub.lock().await.pending_permissions.len(), 1);

    current
        .client
        .respond_permission("main", &interaction.id, Some(interaction.digest), false)
        .await;
    assert_eq!(request.await.unwrap().unwrap()["allow"], false);
}
