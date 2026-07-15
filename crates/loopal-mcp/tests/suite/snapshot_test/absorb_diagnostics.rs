#[tokio::test]
async fn absorb_connections_with_conflicting_tool_names_keeps_both_servers() {
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
    assert_eq!(mgr.collect_snapshots().len(), 1);
}

#[tokio::test]
async fn snapshot_reports_stderr_presence_without_exposing_contents() {
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
            sharing: Default::default(),
            cwd_isolation: None,
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
        !combined.contains(MARKER),
        "snapshot errors must redact stderr values"
    );
    assert!(
        snaps[0]
            .errors
            .iter()
            .any(|e| e.contains("redacted stderr")),
        "stderr presence should remain visible without its contents"
    );
}
