use indexmap::IndexMap;
use loopal_error::McpError;
use loopal_mcp::McpManager;

#[test]
fn test_new_creates_empty_manager() {
    let manager = McpManager::new();
    let tools = manager.get_tools_with_server();
    assert!(tools.is_empty(), "new manager should have no tools");
}

#[test]
fn test_default_creates_empty_manager() {
    let manager = McpManager::default();
    let tools = manager.get_tools_with_server();
    assert!(tools.is_empty());
}

#[test]
fn test_call_tool_unknown_server_returns_error() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let manager = McpManager::new();
        let result = manager
            .call_tool("nonexistent", "some_tool", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            McpError::ServerNotFound(msg) => {
                assert_eq!(msg, "nonexistent");
            }
            other => panic!("expected ServerNotFound, got {other:?}"),
        }
    });
}

#[test]
fn test_call_tool_by_name_unknown_tool_returns_error() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let manager = McpManager::new();
        let result = manager
            .call_tool_by_name("unknown_tool", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            McpError::ServerNotFound(msg) => {
                assert!(msg.contains("unknown_tool"));
            }
            other => panic!("expected ServerNotFound, got {other:?}"),
        }
    });
}

#[test]
fn test_get_tools_with_server_empty_manager_returns_empty() {
    let manager = McpManager::new();
    let tools = manager.get_tools_with_server();
    assert!(
        tools.is_empty(),
        "empty manager should return no tools with server"
    );
}

#[test]
fn test_start_all_empty_configs_succeeds() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut manager = McpManager::new();
        let result = manager.start_all(&IndexMap::new()).await;
        assert!(
            result.is_ok(),
            "start_all with empty configs should succeed"
        );
    });
}
