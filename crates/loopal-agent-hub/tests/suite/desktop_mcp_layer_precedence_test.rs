use loopal_ipc::protocol::methods;
use serde_json::{Value, json};

use super::workspace_rpc_support::setup;

#[tokio::test]
async fn desktop_mcp_list_uses_dot_mcp_precedence_and_preserves_project_secret() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec(&json!({
            "mcp_servers": {"tools": {
                "type": "stdio", "command": "from-settings", "enabled": true,
                "env": {"TOKEN": "settings-secret"}
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join(".mcp.json"),
        serde_json::to_vec(&json!({
            "mcpServers": {"tools": {
                "type": "stdio", "command": "from-dot-mcp", "enabled": false,
                "env": {"TOKEN": "dot-mcp-secret"}
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;

    let listed = conn
        .send_request(
            methods::DESKTOP_LIST_MCP_SERVERS.name,
            json!({
                "workspaceId": "local-workspace"
            }),
        )
        .await
        .unwrap();
    let tools = listed["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|server| server["name"] == "tools")
        .unwrap();
    assert_eq!(tools["command"], "from-dot-mcp");
    assert_eq!(tools["enabled"], false);
    assert_eq!(tools["source"], "project");
    assert!(!listed.to_string().contains("dot-mcp-secret"));

    conn.send_request(
        methods::DESKTOP_UPSERT_MCP_SERVER.name,
        json!({
            "workspaceId": "local-workspace", "server": {
                "type": "stdio", "name": "tools", "command": "edited", "args": [],
                "enabled": true, "timeoutMs": 30000, "sharing": "hub-singleton",
                "cwdIsolation": null, "secretPatches": []
            }
        }),
    )
    .await
    .unwrap();
    let raw: Value =
        serde_json::from_slice(&std::fs::read(dir.join("settings.local.json")).unwrap()).unwrap();
    assert_eq!(
        raw["mcp_servers"]["tools"]["env"]["TOKEN"],
        "dot-mcp-secret"
    );
}

#[tokio::test]
async fn disabling_inherited_server_does_not_copy_or_resurrect_project_secret() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec(&json!({
            "mcp_servers": {"tools": {
                "type": "stdio", "command": "server", "enabled": true,
                "env": {"TOKEN": "project-secret"}
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    let input = |enabled| {
        json!({
            "workspaceId": "local-workspace", "server": {
                "type": "stdio", "name": "tools", "command": "server", "args": [],
                "enabled": enabled, "timeoutMs": 30000, "sharing": "hub-singleton",
                "cwdIsolation": null, "secretPatches": []
            }
        })
    };

    let disabled = conn
        .send_request(methods::DESKTOP_UPSERT_MCP_SERVER.name, input(false))
        .await
        .unwrap();
    let server = disabled["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|server| server["name"] == "tools")
        .unwrap();
    assert_eq!(server["enabled"], false);
    assert_eq!(server["env"], json!([]));
    let raw: Value =
        serde_json::from_slice(&std::fs::read(dir.join("settings.local.json")).unwrap()).unwrap();
    assert!(raw["mcp_servers"]["tools"].get("env").is_none());

    let enabled = conn
        .send_request(methods::DESKTOP_UPSERT_MCP_SERVER.name, input(true))
        .await
        .unwrap();
    let server = enabled["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|server| server["name"] == "tools")
        .unwrap();
    assert_eq!(server["enabled"], true);
    assert_eq!(server["env"], json!([]));
    assert!(!enabled.to_string().contains("project-secret"));
}
