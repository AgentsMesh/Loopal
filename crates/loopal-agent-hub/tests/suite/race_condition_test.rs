//! Regression tests for race conditions in wait_agent and completion chain.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Regression: wait_agent AFTER agent already finished returns cached output.
/// Tests the race where agent finishes before wait_agent is called.
#[tokio::test]
async fn wait_agent_after_finish_returns_cached_output() {
    let (hub, _) = make_hub();

    let (_ca, ct) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(ct));
    let rx = conn.start();
    register_agent_connection(hub.clone(), "fast-agent", conn, rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Agent finishes BEFORE any wait_agent call
    {
        let mut h = hub.lock().await;
        h.registry
            .emit_agent_finished("fast-agent", Some("early result".into()));
        h.registry.unregister_connection("fast-agent");
    }

    // Now call wait_agent — should find cached output, not "not found"
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_WAIT_AGENT.name,
        json!({"name": "fast-agent"}),
        "caller".into(),
    )
    .await
    .unwrap();

    let text = result["output"].as_str().unwrap();
    assert!(
        text.contains("early result"),
        "should find cached output after unregister, got: {text}"
    );
}

/// Regression: watcher set up before agent finishes gets real output.
#[tokio::test]
async fn emit_before_unregister_delivers_output() {
    let (hub, _) = make_hub();

    let (_ca, ct) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(ct));
    let rx = conn.start();
    register_agent_connection(hub.clone(), "normal", conn, rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let hub2 = hub.clone();
    let waiter = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub2,
            methods::HUB_WAIT_AGENT.name,
            json!({"name": "normal"}),
            "parent".into(),
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // emit THEN unregister (correct order)
    {
        let mut h = hub.lock().await;
        h.registry
            .emit_agent_finished("normal", Some("real work done".into()));
        h.registry.unregister_connection("normal");
    }

    let result = tokio::time::timeout(Duration::from_secs(3), waiter).await;
    assert!(result.is_ok(), "waiter should resolve");
    let text = result.unwrap().unwrap().unwrap()["output"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("real work done"), "got: {text}");
}
