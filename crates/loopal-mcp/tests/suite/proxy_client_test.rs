use std::sync::Arc;

use async_trait::async_trait;
use loopal_mcp::{HUB_RPC_BUDGET, HubMcpClient, IpcBudget, McpProvider, McpProxyClient};
use serde_json::{Value, json};

use crate::proxy_client_support::MockHubClient;

#[tokio::test]
async fn list_tools_parses_server_and_tool_definitions() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/list_tools",
        json!({
            "tools": [
                {"server": "s1", "name": "t1", "description": "first", "input_schema": {"type": "object"}},
                {"server": "s2", "name": "t2", "description": "second", "input_schema": {}}
            ]
        }),
    )]);
    let tools = McpProxyClient::new(mock).list_tools(HUB_RPC_BUDGET).await;
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].0, "s1");
    assert_eq!(tools[0].1.name, "t1");
    assert_eq!(tools[1].1.description, "second");
}

#[tokio::test]
async fn list_tools_failures_return_empty_vec() {
    for mock in [
        MockHubClient::new(vec![]),
        MockHubClient::new(vec![("hub/mcp/list_tools", json!({"tools": "invalid"}))]),
    ] {
        assert!(
            McpProxyClient::new(mock)
                .list_tools(HUB_RPC_BUDGET)
                .await
                .is_empty()
        );
    }
}

#[tokio::test]
async fn reconnect_forwards_server_and_requires_connected_response() {
    let success = MockHubClient::new(vec![("hub/mcp/reconnect", json!({"connected": true}))]);
    McpProxyClient::new(success.clone())
        .reconnect("server", HUB_RPC_BUDGET)
        .await
        .unwrap();
    assert_eq!(success.requests()[0].1["server"], "server");

    for response in [json!({"connected": false}), json!({"connected": "yes"})] {
        let client = McpProxyClient::new(MockHubClient::new(vec![("hub/mcp/reconnect", response)]));
        assert!(client.reconnect("server", HUB_RPC_BUDGET).await.is_err());
    }
    assert!(
        McpProxyClient::new(MockHubClient::new(vec![]))
            .reconnect("server", HUB_RPC_BUDGET)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reconnect_respects_forbidden_and_timeout_budgets() {
    let forbidden = McpProxyClient::new(MockHubClient::new(vec![]))
        .reconnect("server", IpcBudget::forbidden())
        .await
        .unwrap_err();
    assert!(format!("{forbidden}").contains("Forbidden"));

    struct HangingHub;
    #[async_trait]
    impl HubMcpClient for HangingHub {
        async fn send_request(&self, _: &str, _: Value) -> Result<Value, String> {
            std::future::pending::<Result<Value, String>>().await
        }
    }
    let timeout = McpProxyClient::new(Arc::new(HangingHub))
        .reconnect(
            "server",
            IpcBudget::allow(std::time::Duration::from_millis(1)),
        )
        .await
        .unwrap_err();
    assert!(format!("{timeout}").contains("timed out"));
}

#[tokio::test]
async fn call_tool_forwards_request_and_error_state() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/call_tool",
        json!({"content": [{"type": "text", "text": "hello"}], "is_error": true}),
    )]);
    let result = McpProxyClient::new(mock.clone())
        .call_tool("srv", "the_tool", &json!({"k": "v"}), HUB_RPC_BUDGET)
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));

    let requests = mock.requests();
    assert_eq!(requests[0].0, "hub/mcp/call_tool");
    assert_eq!(requests[0].1["server"], "srv");
    assert_eq!(requests[0].1["tool"], "the_tool");
    assert_eq!(requests[0].1["args"]["k"], "v");
}

#[tokio::test]
async fn call_tool_transport_error_returns_mcp_error() {
    let error = McpProxyClient::new(MockHubClient::new(vec![]))
        .call_tool("s", "t", &json!({}), HUB_RPC_BUDGET)
        .await
        .unwrap_err();
    assert!(format!("{error}").contains("hub/mcp/call_tool"));
}

#[tokio::test]
async fn snapshot_parses_records_and_rejects_malformed_payload() {
    let valid = MockHubClient::new(vec![(
        "hub/mcp/snapshot",
        json!({"servers": [{
            "name": "s1", "transport": "stdio", "source": "project",
            "status": "connected", "tool_count": 3, "resource_count": 0,
            "prompt_count": 1, "errors": []
        }]}),
    )]);
    let snapshots = McpProxyClient::new(valid).snapshot(HUB_RPC_BUDGET).await;
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].name, "s1");
    assert_eq!(snapshots[0].tool_count, 3);

    let malformed = MockHubClient::new(vec![("hub/mcp/snapshot", json!({"servers": 42}))]);
    assert!(
        McpProxyClient::new(malformed)
            .snapshot(HUB_RPC_BUDGET)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn call_tool_times_out_when_hub_hangs() {
    struct HangingHub;
    #[async_trait]
    impl HubMcpClient for HangingHub {
        async fn send_request(&self, _: &str, _: Value) -> Result<Value, String> {
            std::future::pending::<Result<Value, String>>().await
        }
    }
    let start = std::time::Instant::now();
    let error = McpProxyClient::new(Arc::new(HangingHub))
        .call_tool("s", "t", &json!({}), IpcBudget::allow_secs(1))
        .await
        .unwrap_err();
    assert!(start.elapsed() < std::time::Duration::from_secs(3));
    assert!(format!("{error}").contains("timed out"));
}
