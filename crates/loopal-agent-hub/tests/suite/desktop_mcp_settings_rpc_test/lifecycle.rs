#[tokio::test]
async fn mcp_rpc_upserts_and_deletes_without_returning_secret_values() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.local.json"),
        serde_json::to_vec(&json!({"mcp_servers": {
            "desktop": {
                "type": "stdio", "command": "old", "args": [],
                "env": {"KEEP": "preserved-secret", "REMOVE": "deleted-secret"},
                "enabled": true, "timeout_ms": 30000, "future_field": {"kept": true}
            },
            "other": {"type": "stdio", "command": "other", "enabled": true}
        }}))
        .unwrap(),
    )
    .unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;

    let before = list(&conn).await;
    let desktop = before["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|server| server["name"] == "desktop")
        .unwrap();
    assert_eq!(desktop["env"][0]["configured"], true);
    assert!(!before.to_string().contains("preserved-secret"));

    let updated = conn
        .send_request(
            methods::DESKTOP_UPSERT_MCP_SERVER.name,
            stdio(
                "desktop",
                json!([
                    {"target": "env", "name": "TOKEN", "operation": "set", "value": "new-secret"},
                    {"target": "env", "name": "REMOVE", "operation": "remove"}
                ]),
            ),
        )
        .await
        .unwrap();
    assert!(!updated.to_string().contains("new-secret"));
    assert!(!updated.to_string().contains("preserved-secret"));
    let raw: Value =
        serde_json::from_slice(&std::fs::read(dir.join("settings.local.json")).unwrap()).unwrap();
    assert_eq!(
        raw["mcp_servers"]["desktop"]["env"]["KEEP"],
        "preserved-secret"
    );
    assert_eq!(raw["mcp_servers"]["desktop"]["env"]["TOKEN"], "new-secret");
    assert!(raw["mcp_servers"]["desktop"]["env"].get("REMOVE").is_none());
    assert_eq!(raw["mcp_servers"]["desktop"]["future_field"]["kept"], true);
    assert_eq!(raw["mcp_servers"]["other"]["command"], "other");

    let deleted = conn
        .send_request(
            methods::DESKTOP_DELETE_MCP_SERVER.name,
            json!({"workspaceId": "local-workspace", "name": "desktop"}),
        )
        .await
        .unwrap();
    assert!(
        !deleted["servers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|server| server["name"] == "desktop")
    );
    let raw: Value =
        serde_json::from_slice(&std::fs::read(dir.join("settings.local.json")).unwrap()).unwrap();
    assert_eq!(raw["mcp_servers"]["desktop"]["enabled"], false);
    assert!(raw["mcp_servers"]["desktop"].get("env").is_none());
    assert_eq!(raw["mcp_servers"]["desktop"]["future_field"]["kept"], true);
}

#[tokio::test]
async fn inherited_mcp_secrets_survive_override_and_delete_is_effective() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec(&json!({"mcp_servers": {"remote": {
            "type": "streamable-http",
            "url": "https://user:password@example.test/mcp?token=query-secret",
            "headers": {"Authorization": "Bearer inherited-secret"},
            "enabled": false, "timeout_ms": 30000
        }}}))
        .unwrap(),
    )
    .unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    let before = list(&conn).await;
    let serialized = before.to_string();
    assert!(!serialized.contains("password"));
    assert!(!serialized.contains("query-secret"));
    assert!(!serialized.contains("inherited-secret"));
    assert!(serialized.contains("https://example.test/mcp"));
    assert_eq!(
        before["servers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|server| server["name"] == "remote")
            .unwrap()["enabled"],
        false
    );

    let updated = conn
        .send_request(
            methods::DESKTOP_UPSERT_MCP_SERVER.name,
            json!({
                "workspaceId": "local-workspace", "server": {
                    "type": "streamable-http", "name": "remote", "url": "https://example.test/v2",
                    "enabled": true, "timeoutMs": 10000, "sharing": "hub-singleton",
                    "secretPatches": []
                }
            }),
        )
        .await
        .unwrap();
    assert!(!updated.to_string().contains("inherited-secret"));
    let raw: Value =
        serde_json::from_slice(&std::fs::read(dir.join("settings.local.json")).unwrap()).unwrap();
    assert_eq!(
        raw["mcp_servers"]["remote"]["headers"]["Authorization"],
        "Bearer inherited-secret"
    );

    let deleted = conn
        .send_request(
            methods::DESKTOP_DELETE_MCP_SERVER.name,
            json!({
                "workspaceId": "local-workspace", "name": "remote"
            }),
        )
        .await
        .unwrap();
    assert!(
        !deleted["servers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|server| server["name"] == "remote")
    );
}
