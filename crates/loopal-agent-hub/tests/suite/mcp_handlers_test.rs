use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use loopal_agent_hub::dispatch::dispatch_hub_request;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{AgentEvent, ROOT_AGENT_NAME};
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};

fn make_hub() -> Arc<Mutex<Hub>> {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(16);
    Arc::new(Mutex::new(Hub::new(tx)))
}

/// Spawn a mock root agent: register its hub-facing connection as the
/// `ROOT_AGENT_NAME` agent and drive the mock side responding to `agent/mcp/*`
/// requests with the provided JSON. Drops the mock-side `Connection` only
/// after the test is done — kept alive via the returned task handle.
async fn install_mock_root(
    hub: &Arc<Mutex<Hub>>,
    responder: impl Fn(&str, Value) -> Value + Send + Sync + 'static,
) -> tokio::task::JoinHandle<()> {
    let (hub_side_transport, mock_side_transport) = loopal_ipc::duplex_pair();
    let hub_side = Arc::new(Connection::new(hub_side_transport));
    let _hub_side_rx = hub_side.start();
    let mock_side = Arc::new(Connection::new(mock_side_transport));
    let mut mock_rx = mock_side.start();

    {
        let mut h = hub.lock().await;
        h.registry
            .register_connection(ROOT_AGENT_NAME, hub_side)
            .expect("register root agent");
    }

    let responder = Arc::new(responder);
    tokio::spawn(async move {
        while let Some(msg) = mock_rx.recv().await {
            if let Incoming::Request { id, method, params } = msg {
                let resp = (responder)(&method, params);
                let _ = mock_side.respond(id, resp).await;
            }
        }
    })
}

#[tokio::test]
async fn mcp_list_tools_without_root_returns_error() {
    let hub = make_hub();
    let result = dispatch_hub_request(&hub, "hub/mcp/list_tools", json!({}), "sub-1".into()).await;
    let err = result.unwrap_err();
    assert!(
        err.contains("main") && err.contains("not registered"),
        "expected root-not-registered error, got: {err}"
    );
}

#[tokio::test]
async fn mcp_call_tool_without_root_returns_error() {
    let hub = make_hub();
    let result = dispatch_hub_request(
        &hub,
        "hub/mcp/call_tool",
        json!({"server": "s", "tool": "t", "args": {}}),
        "sub-1".into(),
    )
    .await;
    let err = result.unwrap_err();
    assert!(err.contains("main"));
}

#[tokio::test]
async fn mcp_snapshot_without_root_returns_error() {
    let hub = make_hub();
    let result = dispatch_hub_request(&hub, "hub/mcp/snapshot", json!({}), "sub-1".into()).await;
    let err = result.unwrap_err();
    assert!(err.contains("main"));
}

#[tokio::test]
async fn mcp_list_tools_forwards_to_root_and_returns_payload() {
    let hub = make_hub();
    let _mock = install_mock_root(&hub, |method, _params| match method {
        "agent/mcp/list_tools" => json!({
            "tools": [{
                "server": "test-srv",
                "name": "mock-tool",
                "description": "from mock root",
                "input_schema": {"type": "object"}
            }]
        }),
        other => json!({"error": format!("unexpected method: {other}")}),
    })
    .await;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        dispatch_hub_request(&hub, "hub/mcp/list_tools", json!({}), "sub-1".into()),
    )
    .await
    .expect("must not hang")
    .expect("forward should succeed");

    let tools = result["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["server"], "test-srv");
    assert_eq!(tools[0]["name"], "mock-tool");
}

#[tokio::test]
async fn mcp_call_tool_forwards_params_and_returns_response() {
    let hub = make_hub();
    let _mock = install_mock_root(&hub, |method, params| {
        assert_eq!(method, "agent/mcp/call_tool");
        // Echo back the received params so we can verify forwarding.
        assert_eq!(params["server"], "srv-x");
        assert_eq!(params["tool"], "tool-y");
        assert_eq!(params["args"]["k"], "v");
        json!({
            "content": [{"type": "text", "text": "tool result"}],
            "is_error": false,
        })
    })
    .await;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        dispatch_hub_request(
            &hub,
            "hub/mcp/call_tool",
            json!({"server": "srv-x", "tool": "tool-y", "args": {"k": "v"}}),
            "sub-1".into(),
        ),
    )
    .await
    .expect("must not hang")
    .expect("forward should succeed");

    assert_eq!(result["is_error"], false);
    let content = result["content"].as_array().unwrap();
    assert_eq!(content[0]["text"], "tool result");
}

#[tokio::test]
async fn mcp_snapshot_forwards_and_returns_server_list() {
    let hub = make_hub();
    let _mock = install_mock_root(&hub, |method, _params| {
        assert_eq!(method, "agent/mcp/snapshot");
        json!({
            "servers": [{
                "name": "live-server",
                "transport": "stdio",
                "source": "project",
                "status": "connected",
                "tool_count": 2,
                "resource_count": 0,
                "prompt_count": 0,
                "errors": []
            }]
        })
    })
    .await;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        dispatch_hub_request(&hub, "hub/mcp/snapshot", json!({}), "sub-1".into()),
    )
    .await
    .expect("must not hang")
    .expect("forward should succeed");

    let servers = result["servers"].as_array().unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0]["name"], "live-server");
    assert_eq!(servers[0]["tool_count"], 2);
}

#[tokio::test]
async fn mcp_forward_times_out_when_root_agent_hangs() {
    // Critical anti-leak invariant: JSON-RPC has no cancellation. If hub's
    // `conn.send_request` had no deadline and root agent hung forever, the
    // dispatch task would leak even after sub-agent's proxy_rpc_timeout
    // dropped its future. Verify hub forward fails fast via its own
    // independent timeout.
    let hub = make_hub();
    let (hub_side_transport, mock_side_transport) = loopal_ipc::duplex_pair();
    let hub_side = Arc::new(Connection::new(hub_side_transport));
    let _hub_side_rx = hub_side.start();
    // Mock side: register but NEVER respond. Keep its connection alive so
    // hub's send_request actually waits (vs. immediate transport closed).
    let mock_side = Arc::new(Connection::new(mock_side_transport));
    let _mock_rx = mock_side.start();
    {
        let mut h = hub.lock().await;
        h.registry
            .register_connection(ROOT_AGENT_NAME, hub_side)
            .expect("register");
    }

    // SAFETY: env mutation is process-global; this test runs single-threaded.
    unsafe { std::env::set_var("LOOPAL_HUB_MCP_FORWARD_TIMEOUT_SECS", "1") };
    let start = std::time::Instant::now();
    let result = dispatch_hub_request(&hub, "hub/mcp/list_tools", json!({}), "sub-x".into()).await;
    let elapsed = start.elapsed();
    unsafe { std::env::remove_var("LOOPAL_HUB_MCP_FORWARD_TIMEOUT_SECS") };

    let err = result.unwrap_err();
    assert!(
        err.contains("timed out"),
        "expected timeout error, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "hub must release dispatch task at its own deadline, took {elapsed:?}"
    );
    // Keep mock alive until here so the conn isn't auto-closed by RAII drop
    // before send_request actually attempts the wait.
    drop(mock_side);
}
