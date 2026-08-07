//! Regression tests for race conditions in wait_agent and completion chain.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentCompletion, AgentEvent, WAIT_AGENT_TYPED_RESPONSE_V1};
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Regression: wait_agent AFTER agent already finished returns cached output.
/// Tests the race where agent finishes before wait_agent is called.
#[tokio::test]
async fn wait_agent_after_finish_returns_cached_output() {
    let (hub, _event_rx) = make_hub();

    let (_ca, ct) = loopal_ipc::duplex_pair();
    let (conn, rx) = Connection::new(ct).into_listening();
    let _ = register_agent_connection(hub.clone(), "fast-agent", conn, rx, None, None, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Agent finishes BEFORE any wait_agent call
    {
        let mut h = hub.lock().await;
        let _pending = h
            .registry
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
    assert_eq!(result["status"], "completed");
    assert_eq!(result["reason"], "goal");
    assert!(
        text.contains("early result"),
        "should find cached output after unregister, got: {text}"
    );
}

/// Regression: watcher set up before agent finishes gets real output.
#[tokio::test]
async fn emit_before_unregister_delivers_output() {
    let (hub, _event_rx) = make_hub();

    let (_ca, ct) = loopal_ipc::duplex_pair();
    let (conn, rx) = Connection::new(ct).into_listening();
    let _ = register_agent_connection(hub.clone(), "normal", conn, rx, None, None, None)
        .await
        .unwrap();
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
        let _pending = h
            .registry
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

#[tokio::test]
async fn live_and_cached_failed_waiters_return_the_same_typed_response() {
    let (hub, _event_rx) = make_hub();
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (conn, rx) = Connection::new(transport).into_listening();
    register_agent_connection(hub.clone(), "failing", conn, rx, None, None, None)
        .await
        .unwrap();

    let live_hub = hub.clone();
    let live = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &live_hub,
            methods::HUB_WAIT_AGENT.name,
            json!({
                "name": "failing",
                "response_format": WAIT_AGENT_TYPED_RESPONSE_V1,
            }),
            "parent".into(),
        )
        .await
        .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let mut h = hub.lock().await;
        let _pending = h.registry.emit_agent_completion(
            "failing",
            AgentCompletion::new("error", Some("partial findings".into())),
        );
        h.registry.unregister_connection("failing");
    }

    let live = live.await.unwrap();
    let cached = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_WAIT_AGENT.name,
        json!({
            "name": "failing",
            "response_format": WAIT_AGENT_TYPED_RESPONSE_V1,
        }),
        "parent".into(),
    )
    .await
    .unwrap();
    assert_eq!(live, cached);
    assert_eq!(live["status"], "failed");
    assert_eq!(live["reason"], "error");
    assert_eq!(live["output"], "partial findings");
}

#[tokio::test]
async fn multiple_live_waiters_share_one_completion_channel() {
    let (hub, _event_rx) = make_hub();
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (conn, rx) = Connection::new(transport).into_listening();
    register_agent_connection(hub.clone(), "shared", conn, rx, None, None, None)
        .await
        .unwrap();

    let mut waiters = Vec::new();
    for caller in ["parent-a", "parent-b"] {
        let waiter_hub = hub.clone();
        waiters.push(tokio::spawn(async move {
            loopal_agent_hub::dispatch::dispatch_hub_request(
                &waiter_hub,
                methods::HUB_WAIT_AGENT.name,
                json!({"name": "shared"}),
                caller.into(),
            )
            .await
            .unwrap()
        }));
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let _pending = hub
        .lock()
        .await
        .registry
        .emit_agent_completion("shared", AgentCompletion::goal(Some("one result".into())));

    for waiter in waiters {
        let response = waiter.await.unwrap();
        assert_eq!(response["status"], "completed");
        assert_eq!(response["reason"], "goal");
        assert_eq!(response["output"], "one result");
    }
}

#[tokio::test]
async fn aborted_and_not_found_are_distinct_terminal_responses() {
    let (hub, _event_rx) = make_hub();
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (conn, rx) = Connection::new(transport).into_listening();
    register_agent_connection(hub.clone(), "aborted", conn, rx, None, None, None)
        .await
        .unwrap();
    {
        let mut h = hub.lock().await;
        let _pending = h.registry.emit_agent_completion(
            "aborted",
            AgentCompletion::new("aborted", Some("cancelled by parent".into())),
        );
        h.registry.unregister_connection("aborted");
    }

    let aborted = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_WAIT_AGENT.name,
        json!({"name": "aborted"}),
        "parent".into(),
    )
    .await
    .unwrap();
    let missing = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_WAIT_AGENT.name,
        json!({"name": "missing"}),
        "parent".into(),
    )
    .await
    .unwrap();

    assert_eq!(aborted["status"], "failed");
    assert_eq!(aborted["reason"], "aborted");
    assert_eq!(missing["status"], "not_found");
    assert_eq!(missing["reason"], "not_found");
}
