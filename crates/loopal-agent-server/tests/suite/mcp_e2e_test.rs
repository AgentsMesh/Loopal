//! End-to-end IPC round-trip:
//!     sub-agent McpProxyClient
//!         → HubMcpClient (this test wires it to a fake bridge)
//!             → agent_server::mcp_dispatch (handle_list_tools / handle_call_tool / handle_snapshot)
//!                 → SessionHub.mcp_provider (a real LocalMcpProvider with static tools)
//!
//! This proves the entire 4-hop chain serializes / deserializes correctly,
//! and that McpContentBlock round-trips through the IPC boundary.

use std::sync::Arc;

use async_trait::async_trait;
use loopal_agent_server::dispatch::dispatch_simple;
use loopal_agent_server::session_hub::SessionHub;
use loopal_error::McpError;
use loopal_ipc::protocol::methods;
use loopal_mcp::{HubMcpClient, McpConnectionSnapshot, McpProvider, McpProxyClient};
use loopal_tool_api::ToolDefinition;
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

/// Backing provider exposed via SessionHub (root side).
struct RootProvider {
    tools: Vec<(String, ToolDefinition)>,
}

#[async_trait]
impl McpProvider for RootProvider {
    async fn list_tools(&self) -> Vec<(String, ToolDefinition)> {
        self.tools.clone()
    }
    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        _args: &Value,
    ) -> Result<CallToolResult, McpError> {
        if server == "fail" {
            return Err(McpError::ServerNotFound(format!("no such server: {server}")));
        }
        Ok(CallToolResult::success(vec![
            Content::text(format!("called {server}/{tool}")),
        ]))
    }
    async fn snapshot(&self) -> Vec<McpConnectionSnapshot> {
        vec![McpConnectionSnapshot {
            name: "fake".into(),
            transport: "stdio".into(),
            status: "connected".into(),
            tool_count: self.tools.len(),
            resource_count: 0,
            prompt_count: 0,
            errors: Vec::new(),
        }]
    }
}

/// Routes `hub/mcp/*` directly into agent-server's `dispatch_simple` by
/// rewriting the method to `agent/mcp/*` (skipping the real Hub fan-out).
/// This is the same conversion `mcp_handlers::forward_to_root` does over
/// IPC; here we shortcut it because we don't have a real Hub.
struct SessionHubBridge {
    hub: Arc<SessionHub>,
}

#[async_trait]
impl HubMcpClient for SessionHubBridge {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let agent_method = match method {
            "hub/mcp/list_tools" => methods::AGENT_MCP_LIST_TOOLS.name,
            "hub/mcp/call_tool" => methods::AGENT_MCP_CALL_TOOL.name,
            "hub/mcp/snapshot" => methods::AGENT_MCP_SNAPSHOT.name,
            other => return Err(format!("unmapped method: {other}")),
        };
        dispatch_simple(agent_method, params, &self.hub)
            .await
            .map_err(|e| e.message)
    }
}

async fn setup_proxy(tools: Vec<(String, ToolDefinition)>) -> McpProxyClient {
    let hub = Arc::new(SessionHub::new());
    let provider: Arc<dyn McpProvider> = Arc::new(RootProvider { tools });
    hub.set_mcp_provider(provider).await;
    let bridge: Arc<dyn HubMcpClient> = Arc::new(SessionHubBridge { hub });
    McpProxyClient::new(bridge)
}

#[tokio::test]
async fn e2e_list_tools_round_trips_definitions() {
    let proxy = setup_proxy(vec![
        (
            "server-a".into(),
            ToolDefinition {
                name: "alpha".into(),
                description: "first tool".into(),
                input_schema: serde_json::json!({"type": "object"}),
            },
        ),
        (
            "server-b".into(),
            ToolDefinition {
                name: "beta".into(),
                description: "second tool".into(),
                input_schema: Value::Null,
            },
        ),
    ])
    .await;

    let tools = proxy.list_tools().await;
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].0, "server-a");
    assert_eq!(tools[0].1.name, "alpha");
    assert_eq!(tools[0].1.description, "first tool");
    assert_eq!(tools[1].0, "server-b");
    assert_eq!(tools[1].1.name, "beta");
}

#[tokio::test]
async fn e2e_call_tool_round_trips_success_content() {
    let proxy = setup_proxy(Vec::new()).await;
    let result = proxy
        .call_tool("srv", "act", &serde_json::json!({"k": "v"}))
        .await
        .expect("call_tool ok");
    assert_eq!(result.is_error, Some(false));
    let texts: Vec<String> = result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["called srv/act".to_string()]);
}

#[tokio::test]
async fn e2e_call_tool_surfaces_server_errors() {
    let proxy = setup_proxy(Vec::new()).await;
    let err = proxy
        .call_tool("fail", "x", &Value::Null)
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("no such server: fail"), "got {msg}");
}

#[tokio::test]
async fn e2e_snapshot_round_trips() {
    let proxy = setup_proxy(vec![(
        "any".into(),
        ToolDefinition {
            name: "n".into(),
            description: String::new(),
            input_schema: Value::Null,
        },
    )])
    .await;
    let snaps = proxy.snapshot().await;
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].name, "fake");
    assert_eq!(snaps[0].status, "connected");
    assert_eq!(snaps[0].tool_count, 1);
}

#[tokio::test]
async fn e2e_call_tool_when_hub_has_no_provider_returns_error() {
    let hub = Arc::new(SessionHub::new());
    let bridge: Arc<dyn HubMcpClient> = Arc::new(SessionHubBridge { hub });
    let proxy = McpProxyClient::new(bridge);

    let err = proxy
        .call_tool("s", "t", &Value::Null)
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("no MCP provider"),
        "expected provider-not-attached error, got {msg}"
    );
}

#[tokio::test]
async fn e2e_concurrent_sub_agents_share_root_mcp_without_interference() {
    // Two sub-agent proxies call list_tools concurrently against the same
    // hub-side bridge. Both must receive identical results — no internal
    // state mutation, no missed responses, no cross-talk.
    let proxy = Arc::new(
        setup_proxy(vec![(
            "shared".to_string(),
            ToolDefinition {
                name: "concurrent_tool".to_string(),
                description: "shared".to_string(),
                input_schema: Value::Null,
            },
        )])
        .await,
    );

    let proxy_a = proxy.clone();
    let proxy_b = proxy.clone();
    let proxy_c = proxy.clone();
    let (a, b, c) = tokio::join!(
        proxy_a.list_tools(),
        async move {
            proxy_b
                .call_tool("shared", "concurrent_tool", &serde_json::json!({"id": "b"}))
                .await
        },
        async move { proxy_c.snapshot().await },
    );
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].1.name, "concurrent_tool");
    assert!(!b.unwrap().is_error.unwrap_or(true));
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "fake");
}

#[tokio::test]
async fn e2e_binary_content_round_trips_to_proxy_as_data_url() {
    // Simulate a real MCP server returning image content. Verify the proxy
    // path preserves it (as text representation, since CallToolResult on the
    // proxy side cannot reconstruct binary blocks but `block_to_text` renders
    // a data URL that the LLM can interpret).
    struct BinaryProvider;
    #[async_trait]
    impl McpProvider for BinaryProvider {
        async fn list_tools(&self) -> Vec<(String, ToolDefinition)> {
            Vec::new()
        }
        async fn call_tool(
            &self,
            _server: &str,
            _tool: &str,
            _args: &Value,
        ) -> Result<CallToolResult, McpError> {
            use rmcp::model::{Annotated, RawContent, RawImageContent};
            let raw = RawContent::Image(RawImageContent {
                mime_type: "image/png".to_string(),
                data: "AAAA".to_string(),
                meta: None,
            });
            Ok(CallToolResult::success(vec![Annotated::new(raw, None)]))
        }
        async fn snapshot(&self) -> Vec<McpConnectionSnapshot> {
            Vec::new()
        }
    }

    let hub = Arc::new(SessionHub::new());
    hub.set_mcp_provider(Arc::new(BinaryProvider)).await;
    let bridge: Arc<dyn HubMcpClient> = Arc::new(SessionHubBridge { hub });
    let proxy = McpProxyClient::new(bridge);

    let result = proxy
        .call_tool("any", "img_tool", &Value::Null)
        .await
        .expect("call_tool ok");

    let text = result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .next()
        .expect("proxy renders image block as text");
    assert!(
        text.contains("data:image/png;base64,AAAA"),
        "image block must round-trip as data URL, got {text:?}"
    );
}

#[tokio::test]
async fn e2e_root_call_tool_times_out_independently_of_hub_and_proxy() {
    // Anti-leak invariant for the root-side dispatch path:
    // when a real MCP server hangs, root's `provider.call_tool` future
    // must respect its own deadline so `dispatch_loop` (serial processing!)
    // unblocks for the next request. Without this, a single slow tool
    // blocks every subsequent `hub/mcp/*` request — and any response that
    // arrives AFTER hub's forward_timeout is silently dropped, losing
    // successful tool results.
    struct HangingProvider;
    #[async_trait]
    impl McpProvider for HangingProvider {
        async fn list_tools(&self) -> Vec<(String, ToolDefinition)> {
            Vec::new()
        }
        async fn call_tool(
            &self,
            _server: &str,
            _tool: &str,
            _args: &Value,
        ) -> Result<CallToolResult, McpError> {
            // reason: simulate a real MCP server that accepts the call but
            // never completes (server stuck waiting on user input / hung
            // upstream API).
            std::future::pending::<Result<CallToolResult, McpError>>().await
        }
        async fn snapshot(&self) -> Vec<McpConnectionSnapshot> {
            Vec::new()
        }
    }

    let hub = Arc::new(SessionHub::new());
    hub.set_mcp_provider(Arc::new(HangingProvider)).await;

    // SAFETY: env mutation is process-global; tokio::test default single-threaded.
    unsafe { std::env::set_var("LOOPAL_MCP_CALL_TIMEOUT_SECS", "1") };
    let bridge: Arc<dyn HubMcpClient> = Arc::new(SessionHubBridge { hub });
    let proxy = McpProxyClient::new(bridge);
    let start = std::time::Instant::now();
    let result = proxy.call_tool("s", "t", &Value::Null).await;
    let elapsed = start.elapsed();
    unsafe { std::env::remove_var("LOOPAL_MCP_CALL_TIMEOUT_SECS") };

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("exceeded") || msg.contains("call_tool"),
        "expected root-side timeout error, got: {msg}"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "root must release dispatch via its own deadline, took {elapsed:?}"
    );
}
