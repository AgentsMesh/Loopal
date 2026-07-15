use loopal_ipc::protocol::methods;
use serde_json::{Value, json};

use super::workspace_rpc_support::setup;

const WORKSPACE: &str = "local-workspace";

#[tokio::test]
async fn global_skill_rpc_is_cas_safe_and_projects_all_layers() {
    let root = tempfile::tempdir().unwrap();
    let user = root.path().join(".loopal-user");
    seed_layers(root.path(), &user);
    let (_hub, conn, _incoming) = setup(root.path()).await;

    let unknown = conn
        .send_request(
            methods::DESKTOP_LIST_SKILLS.name,
            json!({"workspaceId": "unknown"}),
        )
        .await
        .unwrap_err();
    assert!(unknown.to_string().to_lowercase().contains("workspace"));

    let listed = request(
        &conn,
        methods::DESKTOP_LIST_SKILLS.name,
        json!({"workspaceId": WORKSPACE}),
    )
    .await;
    let shared = listed["skills"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["name"] == "/shared")
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 3);
    assert_eq!(
        shared
            .iter()
            .filter(|entry| entry["effective"] == true)
            .count(),
        1
    );
    assert_eq!(
        shared
            .iter()
            .find(|entry| entry["effective"] == true)
            .unwrap()["scope"],
        "project"
    );
    let global = shared
        .iter()
        .find(|entry| entry["scope"] == "global")
        .unwrap();
    assert_eq!(global["editable"], true);
    assert_eq!(global["revision"].as_str().unwrap().len(), 64);
    let legacy = listed["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "/legacy-empty")
        .unwrap();
    assert_eq!(legacy["editable"], true);
    assert_eq!(legacy["revision"].as_str().unwrap().len(), 64);
    let legacy_detail = request(
        &conn,
        methods::DESKTOP_GET_SKILL.name,
        json!({"workspaceId": WORKSPACE, "name": "/legacy-empty"}),
    )
    .await;
    assert_eq!(legacy_detail["body"], "");

    let created = request(
        &conn,
        methods::DESKTOP_UPSERT_SKILL.name,
        json!({
            "workspaceId": WORKSPACE, "name": "/fresh", "description": "Fresh",
            "body": "Run $ARGUMENTS"
        }),
    )
    .await;
    assert_eq!(created["source"], "global");
    assert_eq!(created["editable"], true);
    assert_eq!(created["effective"], true);
    let revision = created["revision"].as_str().unwrap();

    let stale = conn
        .send_request(
            methods::DESKTOP_UPSERT_SKILL.name,
            json!({
                "workspaceId": WORKSPACE, "name": "/fresh", "description": "Stale",
                "body": "body", "expectedRevision": "0".repeat(64)
            }),
        )
        .await
        .unwrap_err();
    assert!(stale.to_string().contains("revision conflict"));

    let updated = request(
        &conn,
        methods::DESKTOP_UPSERT_SKILL.name,
        json!({
            "workspaceId": WORKSPACE, "name": "/fresh", "description": "Updated",
            "body": "exact body", "expectedRevision": revision
        }),
    )
    .await;
    let current = updated["revision"].as_str().unwrap();
    let detail = request(
        &conn,
        methods::DESKTOP_GET_SKILL.name,
        json!({"workspaceId": WORKSPACE, "name": "/fresh"}),
    )
    .await;
    assert_eq!(detail["body"], "exact body");
    assert_eq!(detail["revision"], current);

    let deleted = request(
        &conn,
        methods::DESKTOP_DELETE_SKILL.name,
        json!({"workspaceId": WORKSPACE, "name": "/fresh", "expectedRevision": current}),
    )
    .await;
    assert!(
        deleted["skills"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["name"] != "/fresh")
    );
}

#[tokio::test]
async fn plugin_inventory_rpc_returns_metadata_only() {
    let root = tempfile::tempdir().unwrap();
    let user = root.path().join(".loopal-user");
    seed_layers(root.path(), &user);
    let (_hub, conn, _incoming) = setup(root.path()).await;
    let listed = request(
        &conn,
        methods::DESKTOP_LIST_PLUGINS.name,
        json!({"workspaceId": WORKSPACE}),
    )
    .await;
    assert_eq!(listed["plugins"][0]["name"], "base");
    assert_eq!(listed["plugins"][0]["source"], "plugin:base");
    assert_eq!(listed["plugins"][0]["skills"], json!(["/shared"]));
    assert_eq!(listed["plugins"][0]["mcpServers"], json!(["echo"]));
    assert_eq!(listed["plugins"][0]["hookCount"], 1);
    assert!(!listed.to_string().contains("secret-marker"));
}

fn seed_layers(root: &std::path::Path, user: &std::path::Path) {
    let plugin = user.join("plugins/base");
    std::fs::create_dir_all(plugin.join("skills")).unwrap();
    std::fs::write(plugin.join("skills/shared.md"), "Plugin shared").unwrap();
    std::fs::write(
        plugin.join("settings.json"),
        r#"{
          "secret":"secret-marker",
          "mcp_servers":{"echo":{"type":"stdio","command":"echo-server"}},
          "hooks":[{"event":"pre_tool_use","command":"echo hook"}]
        }"#,
    )
    .unwrap();
    std::fs::create_dir_all(user.join("skills")).unwrap();
    std::fs::write(user.join("skills/shared.md"), "Global shared").unwrap();
    std::fs::write(user.join("skills/legacy-empty.md"), "").unwrap();
    std::fs::create_dir_all(root.join(".loopal/skills")).unwrap();
    std::fs::write(root.join(".loopal/skills/shared.md"), "Project shared").unwrap();
}

async fn request(
    conn: &std::sync::Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    method: &str,
    params: Value,
) -> Value {
    conn.send_request(method, params).await.unwrap()
}
