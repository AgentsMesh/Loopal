use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::McpError;
use loopal_kernel::Kernel;
use loopal_mcp::{McpConnectionSnapshot, McpProvider};
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;

struct StaticProvider {
    tools: Vec<(String, ToolDefinition)>,
}

#[async_trait]
impl McpProvider for StaticProvider {
    async fn list_tools(&self) -> Vec<(String, ToolDefinition)> {
        self.tools.clone()
    }
    async fn call_tool(
        &self,
        _server: &str,
        _tool: &str,
        _args: &Value,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            "ok",
        )]))
    }
    async fn snapshot(&self) -> Vec<McpConnectionSnapshot> {
        Vec::new()
    }
}

#[tokio::test]
async fn set_mcp_provider_swaps_to_proxy_and_clears_local() {
    let mut kernel = Kernel::new(Settings::default()).unwrap();

    assert!(
        kernel.mcp_manager().is_some(),
        "fresh kernel should be Local backend"
    );

    let proxy: Arc<dyn McpProvider> = Arc::new(StaticProvider { tools: Vec::new() });
    kernel.set_mcp_provider(proxy);

    assert!(
        kernel.mcp_manager().is_none(),
        "after set_mcp_provider, backend is Proxy → no local manager"
    );
}

#[tokio::test]
async fn set_mcp_provider_then_finalize_registers_proxy_tools() {
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let provider: Arc<dyn McpProvider> = Arc::new(StaticProvider {
        tools: vec![(
            "remote".to_string(),
            ToolDefinition {
                name: "remote_tool".to_string(),
                description: "from proxy".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            },
        )],
    });
    kernel.set_mcp_provider(provider);

    let settled = kernel.finalize_mcp_tools(Duration::from_millis(50)).await;
    assert!(
        settled,
        "proxy backend skips wait_until_settled (no local spawn) — finalize must return true"
    );

    assert!(
        kernel.get_tool("remote_tool").is_some(),
        "proxy-supplied tools must be registered into ToolRegistry"
    );
}

#[tokio::test]
async fn register_mcp_tools_for_server_is_noop_in_proxy_mode() {
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let provider: Arc<dyn McpProvider> = Arc::new(StaticProvider {
        tools: vec![(
            "remote".to_string(),
            ToolDefinition {
                name: "proxy_tool".to_string(),
                description: String::new(),
                input_schema: Value::Null,
            },
        )],
    });
    kernel.set_mcp_provider(provider);

    kernel.register_mcp_tools_for_server("remote").await;
    assert!(
        kernel.get_tool("proxy_tool").is_none(),
        "register_mcp_tools_for_server reads from local manager; proxy mode must no-op"
    );
}

#[tokio::test]
async fn register_mcp_tools_for_server_unknown_local_server_is_silent() {
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel
        .register_mcp_tools_for_server("server-that-does-not-exist")
        .await;
    assert!(kernel.get_tool("anything").is_none());
}

#[tokio::test]
async fn register_mcp_tools_for_server_picks_up_late_connected_tools() {
    // User-facing reconnect flow: user presses 'r' in /mcp page → control
    // McpReconnect → manager.restart_connection → register_mcp_tools_for_server.
    // We can't really run restart_connection (it needs a real subprocess),
    // but we can inject a fake-connected connection directly into the
    // manager and verify the second half — that register_mcp_tools_for_server
    // picks up tools and puts them into ToolRegistry where the LLM finds them.
    use loopal_mcp::{ConnectionStatus, McpConnection};
    let kernel = Kernel::new(Settings::default()).unwrap();

    let manager = kernel.mcp_manager().expect("root kernel has local manager");
    {
        let mut mgr = manager.write().await;
        let mut conn = McpConnection::new(
            "fake-server".to_string(),
            loopal_config::McpServerConfig::Stdio {
                command: "echo".to_string(),
                args: vec![],
                env: Default::default(),
                enabled: true,
                timeout_ms: 1000,
            },
            None,
        );
        conn.status = ConnectionStatus::Connected;
        conn.cached_tools = vec![ToolDefinition {
            name: "late_arrival_tool".to_string(),
            description: "registered after reconnect".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }];
        mgr.absorb_connections(vec![conn])
            .expect("single connected should succeed");
    }

    assert!(
        kernel.get_tool("late_arrival_tool").is_none(),
        "tool should NOT be in ToolRegistry yet — only after register_mcp_tools_for_server"
    );

    kernel.register_mcp_tools_for_server("fake-server").await;

    assert!(
        kernel.get_tool("late_arrival_tool").is_some(),
        "reconnect handler must surface the server's tools to LLM via ToolRegistry"
    );
}

#[tokio::test]
async fn register_all_settled_mcp_tools_is_idempotent_on_reentry() {
    // The late-registration listener triggers register_all_settled_mcp_tools
    // every time mark_settled fires. If multiple spawn_background events
    // occur (e.g. user toggles a server), the same tool name must not
    // double-register or panic.
    use loopal_mcp::{ConnectionStatus, McpConnection};
    let kernel = Kernel::new(Settings::default()).unwrap();

    let manager = kernel.mcp_manager().expect("local manager");
    {
        let mut mgr = manager.write().await;
        let mut conn = McpConnection::new(
            "srv".to_string(),
            loopal_config::McpServerConfig::Stdio {
                command: "echo".to_string(),
                args: vec![],
                env: Default::default(),
                enabled: true,
                timeout_ms: 1000,
            },
            None,
        );
        conn.status = ConnectionStatus::Connected;
        conn.cached_tools = vec![ToolDefinition {
            name: "shared_tool".to_string(),
            description: String::new(),
            input_schema: Value::Null,
        }];
        mgr.absorb_connections(vec![conn]).unwrap();
    }

    kernel.register_all_settled_mcp_tools().await;
    assert!(kernel.get_tool("shared_tool").is_some());

    // Second call must be a no-op, not a panic / duplicate / replacement.
    kernel.register_all_settled_mcp_tools().await;
    assert!(kernel.get_tool("shared_tool").is_some());
}
