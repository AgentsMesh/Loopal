use std::sync::Arc;

use loopal_protocol::AgentEventPayload;
use loopal_runtime::frontend::{ManualPermissionHandler, PermissionHandler};
use loopal_tool_api::PermissionDecision;
use tokio::sync::mpsc;

use super::permission_request_support::permission_request;

#[tokio::test]
async fn permission_handler_allows_approval() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (permission_tx, permission_rx) = mpsc::channel(16);
    let handler = Arc::new(ManualPermissionHandler::new(event_tx, permission_rx));
    let request = permission_request("id1", "Write", serde_json::json!({}));

    tokio::spawn(async move {
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(
            event.payload,
            AgentEventPayload::ToolPermissionRequest { .. }
        ));
        permission_tx.send(true).await.unwrap();
    });

    let outcome = handler.decide(&request).await;
    assert_eq!(outcome.decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn permission_handler_denies_rejection() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (permission_tx, permission_rx) = mpsc::channel(16);
    let handler = ManualPermissionHandler::new(event_tx, permission_rx);
    let request = permission_request("id1", "Write", serde_json::json!({}));

    tokio::spawn(async move {
        let _ = event_rx.recv().await;
        permission_tx.send(false).await.unwrap();
    });

    let outcome = handler.decide(&request).await;
    assert_eq!(outcome.decision, PermissionDecision::Deny);
}

#[tokio::test]
async fn permission_handler_denies_closed_event_channel() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_permission_tx, permission_rx) = mpsc::channel(16);
    drop(event_rx);
    let handler = ManualPermissionHandler::new(event_tx, permission_rx);
    let request = permission_request("id1", "Write", serde_json::json!({}));

    let outcome = handler.decide(&request).await;
    assert_eq!(outcome.decision, PermissionDecision::Deny);
}
