#[tokio::test]
async fn mcp_rpc_rejects_unsafe_names_urls_and_secret_operations() {
    let root = tempfile::tempdir().unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    for input in [
        stdio("bad.name", json!([])),
        stdio(
            "safe",
            json!([{
                "target": "env", "name": "TOKEN", "operation": "remove", "value": "must-not-echo"
            }]),
        ),
        json!({"workspaceId": "local-workspace", "server": {
            "type": "streamable-http", "name": "remote",
            "url": "https://user:secret@example.test/mcp", "enabled": true,
            "timeoutMs": 30000, "sharing": "spawn-tree", "secretPatches": []
        }}),
    ] {
        let error = conn
            .send_request(methods::DESKTOP_UPSERT_MCP_SERVER.name, input)
            .await
            .expect_err("unsafe MCP definition must fail");
        assert!(!error.to_string().contains("must-not-echo"));
    }
    let dir = root.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec(&json!({
            "mcp_servers": {"remote": {
                "type": "streamable-http", "url": "https://example.test/mcp",
                "headers": {"X.Token": "dot-secret"}, "enabled": true
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    let error = conn
        .send_request(
            methods::DESKTOP_UPSERT_MCP_SERVER.name,
            json!({
                "workspaceId": "local-workspace", "server": {
                    "type": "streamable-http", "name": "remote", "url": "https://example.test/v2",
                    "enabled": true, "timeoutMs": 30000, "sharing": "hub-singleton",
                    "secretPatches": []
                }
            }),
        )
        .await
        .expect_err("dotted inherited header must fail safely");
    assert!(error.to_string().contains("cannot be safely edited"));
    assert!(!root.path().join(".loopal/settings.local.json").exists());
    conn.send_request(
        methods::DESKTOP_LIST_MCP_SERVERS.name,
        json!({"workspaceId": "outside-workspace"}),
    )
    .await
    .expect_err("cross-workspace MCP list must fail");
}

#[tokio::test]
async fn mcp_list_skips_legacy_fields_that_cannot_cross_the_typed_contract() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    let long_key = "K".repeat(129);
    let long_name = "s".repeat(65);
    std::fs::write(
        dir.join("settings.local.json"),
        serde_json::to_vec(&json!({
            "mcp_servers": {
                "valid": {"type": "stdio", "command": "node", "enabled": true,
                    "env": {"GOOD": "secret", long_key: "hidden"}},
                long_name.clone(): {"type": "stdio", "command": "node", "enabled": true},
                "bad_command": {"type": "stdio", "command": "node\nsecret", "enabled": true}
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    let response = list(&conn).await;
    let servers = response["servers"].as_array().unwrap();
    assert!(!servers.iter().any(|server| server["name"] == long_name));
    assert!(!servers.iter().any(|server| server["name"] == "bad_command"));
    let valid = servers
        .iter()
        .find(|server| server["name"] == "valid")
        .unwrap();
    assert_eq!(valid["env"], json!([{"name": "GOOD", "configured": true}]));
    assert!(!response.to_string().contains("secret"));
}
