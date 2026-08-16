use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, UiSession, hub_server};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, UiCapabilities};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn full_event_queue_delivers_resolved_before_agent_response() {
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(1);
    let hub = crate::permission_support::hub_with_noop_audit(event_tx);
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _incoming) = hub_server::connect_local(hub.clone(), "main");

    let request = tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request("queue-full", "Bash", json!({"command": "pwd"})),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while !hub
            .lock()
            .await
            .pending_permissions
            .contains_key(&("main".into(), "queue-full".into()))
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let interaction = crate::permission_interaction(&hub, "main", "queue-full").await;

    ui.client
        .respond_permission("main", &interaction.id, Some(interaction.digest), true)
        .await;
    tokio::task::yield_now().await;
    assert!(
        !request.is_finished(),
        "agent response must wait while Resolved cannot enter the full queue"
    );

    let queued_request = event_rx.recv().await.unwrap();
    assert!(matches!(
        queued_request.payload,
        AgentEventPayload::ToolPermissionRequest { .. }
    ));
    let terminal = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        terminal.payload,
        AgentEventPayload::ToolPermissionResolved { ref id } if id == &interaction.id
    ));

    let response = tokio::time::timeout(Duration::from_secs(1), request)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(response["allow"], true);
}

#[tokio::test]
async fn closed_terminal_queue_invalidates_hub_instead_of_responding_silently() {
    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(1);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _incoming) = hub_server::connect_local(hub.clone(), "main");
    let request = tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request("closed-terminal-queue", "Bash", json!({})),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while !hub
            .lock()
            .await
            .pending_permissions
            .contains_key(&("main".into(), "closed-terminal-queue".into()))
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let interaction = crate::permission_interaction(&hub, "main", "closed-terminal-queue").await;

    drop(event_rx);
    let shutdown = hub.lock().await.shutdown_signal.clone();
    ui.client
        .respond_permission("main", &interaction.id, Some(interaction.digest), true)
        .await;
    tokio::time::timeout(Duration::from_secs(1), shutdown.notified())
        .await
        .expect("terminal queue failure must explicitly invalidate the Hub");
    let outcome = tokio::time::timeout(Duration::from_secs(1), request)
        .await
        .unwrap()
        .unwrap();
    assert!(
        outcome.is_err(),
        "agent must not receive an unobservable success"
    );
}
