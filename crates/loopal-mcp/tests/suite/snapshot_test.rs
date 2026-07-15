use loopal_mcp::{ConnectionStatus, McpConnection, McpManager};

fn fake_connected_conn(name: &str, tool_names: &[&str]) -> McpConnection {
    use loopal_config::McpServerConfig;
    use loopal_tool_api::ToolDefinition;
    let config = McpServerConfig::Stdio {
        command: "echo".to_string(),
        args: vec![],
        env: Default::default(),
        enabled: true,
        timeout_ms: 1000,
        sharing: Default::default(),
        cwd_isolation: None,
    };
    let mut conn = McpConnection::new(name.to_string(), config, None);
    conn.status = ConnectionStatus::Connected;
    conn.cached_tools = tool_names
        .iter()
        .map(|t| ToolDefinition {
            name: t.to_string(),
            description: String::new(),
            input_schema: serde_json::Value::Null,
        })
        .collect();
    conn
}

include!("snapshot_test/manager_states.rs");
include!("snapshot_test/absorb_diagnostics.rs");
