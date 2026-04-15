use loopal_mcp::McpManager;

#[test]
fn test_collect_snapshots_empty_manager() {
    let manager = McpManager::new();
    let snapshots = manager.collect_snapshots();
    assert!(snapshots.is_empty());
}

#[test]
fn test_get_tools_for_server_unknown() {
    let manager = McpManager::new();
    let tools = manager.get_tools_for_server("nonexistent");
    assert!(tools.is_empty());
}

#[test]
fn test_get_server_instructions_empty() {
    let manager = McpManager::new();
    assert!(manager.get_server_instructions().is_empty());
}

#[test]
fn test_get_resources_empty() {
    let manager = McpManager::new();
    assert!(manager.get_resources().is_empty());
}

#[test]
fn test_get_prompts_empty() {
    let manager = McpManager::new();
    assert!(manager.get_prompts().is_empty());
}

#[tokio::test]
async fn test_restart_connection_unknown_server() {
    let mut manager = McpManager::new();
    let result = manager.restart_connection("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_start_all_with_failed_server_keeps_connection() {
    use indexmap::IndexMap;
    use loopal_config::McpServerConfig;

    let mut manager = McpManager::new();
    let mut configs = IndexMap::new();
    configs.insert(
        "bad-server".to_string(),
        McpServerConfig::Stdio {
            command: "__nonexistent_mcp__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 2000,
        },
    );
    // All servers fail → returns Err, but connections are still populated.
    let result = manager.start_all(&configs).await;
    assert!(result.is_err());

    let snapshots = manager.collect_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].name, "bad-server");
    assert_eq!(snapshots[0].transport, "stdio");
    assert!(snapshots[0].status.starts_with("failed"));
    assert!(!snapshots[0].errors.is_empty());
    assert_eq!(snapshots[0].tool_count, 0);
}

#[tokio::test]
async fn test_get_tools_for_server_failed_returns_empty() {
    use indexmap::IndexMap;
    use loopal_config::McpServerConfig;

    let mut manager = McpManager::new();
    let mut configs = IndexMap::new();
    configs.insert(
        "bad".to_string(),
        McpServerConfig::Stdio {
            command: "__nonexistent__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 2000,
        },
    );
    let _ = manager.start_all(&configs).await;
    let tools = manager.get_tools_for_server("bad");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn test_restart_connection_on_failed_server() {
    use indexmap::IndexMap;
    use loopal_config::McpServerConfig;

    let mut manager = McpManager::new();
    let mut configs = IndexMap::new();
    configs.insert(
        "bad".to_string(),
        McpServerConfig::Stdio {
            command: "__nonexistent__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 2000,
        },
    );
    let _ = manager.start_all(&configs).await;

    // Restart also fails (same bad command), but should not panic.
    let result = manager.restart_connection("bad").await;
    assert!(result.is_ok());

    let snapshots = manager.collect_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].status.starts_with("failed"));
}

#[tokio::test]
async fn test_start_all_disabled_server_skipped() {
    use indexmap::IndexMap;
    use loopal_config::McpServerConfig;

    let mut manager = McpManager::new();
    let mut configs = IndexMap::new();
    configs.insert(
        "disabled".to_string(),
        McpServerConfig::Stdio {
            command: "echo".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: false,
            timeout_ms: 2000,
        },
    );
    let result = manager.start_all(&configs).await;
    assert!(result.is_ok());
    assert!(manager.collect_snapshots().is_empty());
}
