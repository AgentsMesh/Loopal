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

#[tokio::test]
async fn absorb_connections_with_conflicting_tool_names_keeps_both_servers() {
    // Two MCP servers both register `dup_tool`. Manager keeps both connection
    // entries (so /mcp page can show them) but tool_map points to the LAST
    // absorbed server. ToolRegistry (in kernel) takes the FIRST wins — this
    // test pins down manager-level behavior so the divergence is documented.
    let mut mgr = McpManager::new();
    let conn_a = fake_connected_conn("server-a", &["dup_tool", "uniq_a"]);
    let conn_b = fake_connected_conn("server-b", &["dup_tool", "uniq_b"]);
    mgr.absorb_connections(vec![conn_a, conn_b])
        .expect("both connected");

    let names: Vec<String> = mgr
        .collect_snapshots()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(names.contains(&"server-a".to_string()));
    assert!(names.contains(&"server-b".to_string()));

    // get_tools_for_server reads per-connection cached_tools and is unaffected
    // by tool_map's "last wins" — each server still owns its own tool list.
    assert_eq!(
        mgr.get_tools_for_server("server-a").len(),
        2,
        "server-a keeps both its tools regardless of name conflicts"
    );
    assert_eq!(mgr.get_tools_for_server("server-b").len(), 2);
}

#[tokio::test]
async fn absorb_connections_returns_err_when_all_failed() {
    let mut mgr = McpManager::new();
    let mut failed = fake_connected_conn("bad", &[]);
    failed.status = ConnectionStatus::Failed("boom".into());
    let result = mgr.absorb_connections(vec![failed]);
    assert!(result.is_err(), "all-failed should propagate as error");
    // But the failed connection IS persisted for /mcp page diagnostics.
    assert_eq!(mgr.collect_snapshots().len(), 1);
}

#[tokio::test]
async fn snapshot_surfaces_stderr_tail_for_failed_stdio_server() {
    // User-visibility regression: when an MCP server fails to start, the
    // /mcp page should show the server's own stderr output (not just a
    // generic "did not complete handshake"). This is the exact diagnostic
    // path that lets users debug things like the chrome-devtools-mcp
    // SingletonLock issue on macmini-03-64.
    //
    // Use `sh -c` to spawn a server that writes a distinctive stderr line
    // then exits without speaking MCP — exactly the shape of a real
    // misconfigured/conflicting server.
    use indexmap::IndexMap;
    use loopal_config::McpServerConfig;

    const MARKER: &str = "PROFILE-LOCK-MARKER-FOR-TEST";

    let mut mgr = McpManager::new();
    let mut configs = IndexMap::new();
    configs.insert(
        "diag".to_string(),
        McpServerConfig::Stdio {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), format!("echo '{MARKER}' >&2; sleep 0.5")],
            env: Default::default(),
            enabled: true,
            timeout_ms: 800,
        },
    );

    let _ = mgr.start_all(&configs).await;

    let snaps = mgr.collect_snapshots();
    assert_eq!(snaps.len(), 1);
    assert!(
        snaps[0].status.starts_with("failed"),
        "sleep-only server fails handshake; got {:?}",
        snaps[0].status
    );

    let combined = snaps[0].errors.join(" | ");
    assert!(
        combined.contains(MARKER),
        "snapshot.errors MUST surface the server's stderr so /mcp page can \
         diagnose user-visible failures. Got: {combined}"
    );
    assert!(
        snaps[0].errors.iter().any(|e| e.starts_with("stderr: ")),
        "stderr lines should be prefixed for clarity"
    );
}
