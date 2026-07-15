use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_config::{McpServerConfig, Settings};
use loopal_error::McpError;
use loopal_kernel::Kernel;
use loopal_mcp::{IpcBudget, McpConnectionSnapshot, McpProvider};
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;

struct ProxyFixture {
    status: &'static str,
    tools: Vec<(String, ToolDefinition)>,
}

#[async_trait]
impl McpProvider for ProxyFixture {
    async fn list_tools(&self, _: IpcBudget) -> Vec<(String, ToolDefinition)> {
        if self.status == "connected" {
            self.tools.clone()
        } else {
            Vec::new()
        }
    }

    async fn call_tool(
        &self,
        _: &str,
        _: &str,
        _: &Value,
        _: IpcBudget,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(Vec::new()))
    }

    async fn snapshot(&self, _: IpcBudget) -> Vec<McpConnectionSnapshot> {
        vec![McpConnectionSnapshot {
            name: "fixture".into(),
            transport: "stdio".into(),
            status: self.status.into(),
            tool_count: self.tools.len(),
            resource_count: 0,
            prompt_count: 0,
            errors: Vec::new(),
        }]
    }
}

fn settings() -> Settings {
    let mut settings = Settings::default();
    settings.mcp_servers.insert(
        "fixture".into(),
        McpServerConfig::Stdio {
            command: "fixture".into(),
            args: Vec::new(),
            env: Default::default(),
            enabled: true,
            timeout_ms: 1_000,
            sharing: Default::default(),
            cwd_isolation: None,
        },
    );
    settings
}

#[tokio::test]
async fn connected_zero_tool_proxy_is_settled() {
    let mut kernel = Kernel::new(settings()).unwrap();
    kernel.set_mcp_provider(Arc::new(ProxyFixture {
        status: "connected",
        tools: Vec::new(),
    }));
    assert!(kernel.finalize_mcp_tools(Duration::from_millis(50)).await);
}

#[tokio::test]
async fn connecting_proxy_soft_times_out_without_registering_tools() {
    let mut kernel = Kernel::new(settings()).unwrap();
    kernel.set_mcp_provider(Arc::new(ProxyFixture {
        status: "connecting",
        tools: vec![(
            "fixture".into(),
            ToolDefinition {
                name: "fixture_echo".into(),
                description: String::new(),
                input_schema: Value::Null,
            },
        )],
    }));
    assert!(!kernel.finalize_mcp_tools(Duration::from_millis(10)).await);
    assert!(kernel.get_tool("fixture_echo").is_none());
}

#[tokio::test]
async fn connected_proxy_registers_tools_before_ready() {
    let mut kernel = Kernel::new(settings()).unwrap();
    kernel.set_mcp_provider(Arc::new(ProxyFixture {
        status: "connected",
        tools: vec![(
            "fixture".into(),
            ToolDefinition {
                name: "fixture_echo".into(),
                description: String::new(),
                input_schema: Value::Null,
            },
        )],
    }));
    assert!(kernel.finalize_mcp_tools(Duration::from_millis(50)).await);
    assert!(kernel.get_tool("fixture_echo").is_some());
}
