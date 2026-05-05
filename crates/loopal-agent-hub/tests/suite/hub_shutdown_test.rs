use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::dispatch::dispatch_hub_request;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> Arc<Mutex<Hub>> {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(16);
    Arc::new(Mutex::new(Hub::new(tx)))
}

#[tokio::test]
async fn hub_shutdown_request_returns_ok() {
    let hub = make_hub();
    let result = dispatch_hub_request(&hub, "hub/shutdown", json!({}), "tui".into())
        .await
        .unwrap();
    assert_eq!(result["ok"], true);
}

#[tokio::test]
async fn hub_shutdown_signals_already_registered_waiter() {
    let hub = make_hub();
    let signal = hub.lock().await.shutdown_signal.clone();

    let waiter = tokio::spawn(async move {
        signal.notified().await;
        true
    });

    tokio::task::yield_now().await;

    dispatch_hub_request(&hub, "hub/shutdown", json!({}), "tui".into())
        .await
        .unwrap();

    let fired = tokio::time::timeout(Duration::from_millis(500), waiter)
        .await
        .expect("waiter must wake within 500ms")
        .expect("join handle ok");
    assert!(fired);
}

#[tokio::test]
async fn hub_shutdown_permit_persists_for_late_waiter() {
    // Regression for the notify_waiters() race: hub_only races with
    // a fast TUI sending hub/shutdown before notified().await registers.
    // A stored permit from notify_one() must let the late waiter wake.
    let hub = make_hub();

    dispatch_hub_request(&hub, "hub/shutdown", json!({}), "tui".into())
        .await
        .unwrap();

    let signal = hub.lock().await.shutdown_signal.clone();
    let result = tokio::time::timeout(Duration::from_millis(500), signal.notified()).await;
    assert!(
        result.is_ok(),
        "stored permit must wake a late waiter; if this hangs, signal was lost"
    );
}

#[tokio::test]
async fn hub_shutdown_can_fire_repeatedly() {
    let hub = make_hub();
    for _ in 0..3 {
        let result = dispatch_hub_request(&hub, "hub/shutdown", json!({}), "tui".into()).await;
        assert!(result.is_ok());
    }
}
