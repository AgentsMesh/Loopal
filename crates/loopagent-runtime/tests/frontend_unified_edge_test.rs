use std::sync::Arc;

use loopagent_runtime::frontend::{PermissionHandler, TuiPermissionHandler};
use loopagent_types::event::AgentEventPayload;
use loopagent_types::permission::PermissionDecision;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_tui_permission_handler_approved() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (perm_tx, perm_rx) = mpsc::channel(16);

    let handler = Arc::new(TuiPermissionHandler::new(event_tx, perm_rx));
    let handler_clone = Arc::clone(&handler);

    tokio::spawn(async move {
        let ev = event_rx.recv().await.unwrap();
        assert!(matches!(ev.payload, AgentEventPayload::ToolPermissionRequest { .. }));
        perm_tx.send(true).await.unwrap();
    });

    let d = handler_clone.decide("id1", "Write", &serde_json::json!({})).await;
    assert_eq!(d, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_tui_permission_handler_denied() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (perm_tx, perm_rx) = mpsc::channel(16);

    let handler = TuiPermissionHandler::new(event_tx, perm_rx);

    tokio::spawn(async move {
        let _ = event_rx.recv().await;
        perm_tx.send(false).await.unwrap();
    });

    let d = handler.decide("id1", "Write", &serde_json::json!({})).await;
    assert_eq!(d, PermissionDecision::Deny);
}

#[tokio::test]
async fn test_tui_permission_handler_closed_channel_denies() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_perm_tx, perm_rx) = mpsc::channel(16);
    drop(event_rx); // close the event receiver

    let handler = TuiPermissionHandler::new(event_tx, perm_rx);
    let d = handler.decide("id1", "Write", &serde_json::json!({})).await;
    assert_eq!(d, PermissionDecision::Deny);
}
