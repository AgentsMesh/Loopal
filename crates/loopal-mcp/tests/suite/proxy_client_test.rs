use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_mcp::{HubMcpClient, McpProvider, McpProxyClient};
use serde_json::{Value, json};

struct MockHubClient {
    responses: Mutex<Vec<(String, Value)>>,
    requests: Mutex<Vec<(String, Value)>>,
}

impl MockHubClient {
    fn new(responses: Vec<(&str, Value)>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|(m, v)| (m.to_string(), v))
                    .collect(),
            ),
            requests: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl HubMcpClient for MockHubClient {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.requests
            .lock()
            .unwrap()
            .push((method.to_string(), params));
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(format!("no mock response for {method}"));
        }
        let (expected, resp) = responses.remove(0);
        assert_eq!(expected, method);
        Ok(resp)
    }
}

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
    let proxy = McpProxyClient::new(mock);
    let tools = proxy.list_tools().await;
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].0, "s1");
    assert_eq!(tools[0].1.name, "t1");
    assert_eq!(tools[1].0, "s2");
    assert_eq!(tools[1].1.description, "second");
}

#[tokio::test]
async fn list_tools_empty_response_returns_empty_vec() {
    let mock = MockHubClient::new(vec![("hub/mcp/list_tools", json!({"tools": []}))]);
    let proxy = McpProxyClient::new(mock);
    assert!(proxy.list_tools().await.is_empty());
}

#[tokio::test]
async fn list_tools_ipc_error_returns_empty_vec() {
    let mock = MockHubClient::new(vec![]);
    let proxy = McpProxyClient::new(mock);
    assert!(proxy.list_tools().await.is_empty());
}

#[tokio::test]
async fn call_tool_forwards_server_tool_and_args() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/call_tool",
        json!({"content": [{"type": "text", "text": "hello"}], "is_error": false}),
    )]);
    let proxy = McpProxyClient::new(mock.clone());
    let result = proxy
        .call_tool("srv", "the_tool", &json!({"k": "v"}))
        .await
        .expect("call_tool ok");
    assert_eq!(result.is_error, Some(false));

    let requests = mock.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "hub/mcp/call_tool");
    assert_eq!(requests[0].1["server"], "srv");
    assert_eq!(requests[0].1["tool"], "the_tool");
    assert_eq!(requests[0].1["args"]["k"], "v");
}

#[tokio::test]
async fn call_tool_is_error_true_is_preserved() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/call_tool",
        json!({"content": [{"type": "text", "text": "oops"}], "is_error": true}),
    )]);
    let proxy = McpProxyClient::new(mock);
    let result = proxy
        .call_tool("s", "t", &json!({}))
        .await
        .expect("call_tool ok");
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn call_tool_transport_error_returns_mcp_error() {
    let mock = MockHubClient::new(vec![]);
    let proxy = McpProxyClient::new(mock);
    let err = proxy.call_tool("s", "t", &json!({})).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("hub/mcp/call_tool"));
}

#[tokio::test]
async fn snapshot_parses_server_records() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/snapshot",
        json!({
            "servers": [
                {
                    "name": "s1",
                    "transport": "stdio",
                    "source": "project",
                    "status": "connected",
                    "tool_count": 3,
                    "resource_count": 0,
                    "prompt_count": 1,
                    "errors": []
                }
            ]
        }),
    )]);
    let proxy = McpProxyClient::new(mock);
    let snaps = proxy.snapshot().await;
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].name, "s1");
    assert_eq!(snaps[0].transport, "stdio");
    assert_eq!(snaps[0].tool_count, 3);
}

#[tokio::test]
async fn call_tool_times_out_when_hub_hangs() {
    struct HangingHub;
    #[async_trait::async_trait]
    impl HubMcpClient for HangingHub {
        async fn send_request(&self, _: &str, _: Value) -> Result<Value, String> {
            // reason: simulate a hub that accepts the request but never responds.
            std::future::pending::<Result<Value, String>>().await
        }
    }
    // SAFETY: env override is process-global; this test is single-threaded.
    unsafe {
        std::env::set_var("LOOPAL_MCP_PROXY_RPC_TIMEOUT_SECS", "1");
    }
    let proxy = McpProxyClient::new(Arc::new(HangingHub));
    let start = std::time::Instant::now();
    let err = proxy.call_tool("s", "t", &json!({})).await.unwrap_err();
    let elapsed = start.elapsed();
    unsafe {
        std::env::remove_var("LOOPAL_MCP_PROXY_RPC_TIMEOUT_SECS");
    }
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "proxy must surface timeout, got {elapsed:?}"
    );
    assert!(format!("{err}").contains("timed out"));
}

#[tokio::test]
async fn call_tool_round_trips_non_text_content_blocks() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/call_tool",
        json!({
            "content": [
                {"type": "text", "text": "preamble"},
                {"type": "image", "mime_type": "image/png", "data": "AAAA"},
                {"type": "audio", "mime_type": "audio/wav"},
                {"type": "resource", "uri": "file:///x.md", "text": "hi"},
                {"type": "resource_link", "uri": "https://example.com/spec"}
            ],
            "is_error": false
        }),
    )]);
    let proxy = McpProxyClient::new(mock);
    let result = proxy
        .call_tool("s", "t", &json!({}))
        .await
        .expect("call_tool ok");

    let raw_texts: Vec<String> = result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        raw_texts.len(),
        5,
        "all 5 blocks must survive IPC round-trip and be rendered as text"
    );
    assert_eq!(raw_texts[0], "preamble");
    assert!(
        raw_texts[1].contains("data:image/png;base64,AAAA"),
        "image block should render as data URL, got {:?}",
        raw_texts[1]
    );
    assert!(raw_texts[2].contains("audio/wav"));
    assert!(raw_texts[3].contains("file:///x.md") && raw_texts[3].contains("hi"));
    assert!(raw_texts[4].contains("https://example.com/spec"));
}

#[tokio::test]
async fn list_tools_ipc_returns_empty_on_malformed_payload() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/list_tools",
        json!({"tools": "not-an-array"}),
    )]);
    let proxy = McpProxyClient::new(mock);
    assert!(proxy.list_tools().await.is_empty());
}

#[tokio::test]
async fn snapshot_ipc_returns_empty_on_malformed_payload() {
    let mock = MockHubClient::new(vec![("hub/mcp/snapshot", json!({"servers": 42}))]);
    let proxy = McpProxyClient::new(mock);
    assert!(proxy.snapshot().await.is_empty());
}
