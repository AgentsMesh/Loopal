use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use serde_json::json;

use super::workspace_rpc_support::setup;

fn settings() -> serde_json::Value {
    json!({
        "model": "model", "modelRouting": {
            "default": "", "summarization": "", "classification": "", "refine": ""
        },
        "permissionMode": "bypass", "decisionMode": "manual",
        "sandboxPolicy": "default_write", "thinking": {"type": "auto"},
        "maxContextTokens": 0, "memoryEnabled": true, "microcompactIdleMinutes": 60,
        "telemetryEnabled": true, "outputStyle": ""
    })
}

async fn update(
    conn: &Arc<Connection<Listening>>,
    provider: serde_json::Value,
) -> serde_json::Value {
    conn.send_request(
        methods::DESKTOP_UPDATE_SETTINGS.name,
        json!({
            "workspaceId": "local-workspace", "settings": settings(),
            "providerUpdates": {"openai": provider}
        }),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn provider_disable_enable_and_remove_have_distinct_layer_semantics() {
    let root = tempfile::tempdir().unwrap();
    let config_dir = root.path().join(".loopal-user");
    let plugin_dir = config_dir.join("plugins/base");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(
        plugin_dir.join("settings.json"),
        serde_json::to_vec(&json!({
            "providers": {"openai": {
                "api_key": "inherited-provider-value", "base_url": "https://api.example.test/v1"
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;

    let disabled = update(&conn, json!({"enabled": false})).await;
    assert_eq!(disabled["providers"]["openai"]["enabled"], false);
    assert!(!disabled.to_string().contains("inherited-provider-value"));
    let local = std::fs::read(config_dir.join("settings.json")).unwrap();
    let local: serde_json::Value = serde_json::from_slice(&local).unwrap();
    assert!(local["providers"]["openai"].is_null());

    let enabled = update(&conn, json!({"enabled": true})).await;
    assert_eq!(enabled["providers"]["openai"]["enabled"], true);
    assert_eq!(enabled["providers"]["openai"]["apiKeyConfigured"], true);
    assert!(!enabled.to_string().contains("inherited-provider-value"));

    update(&conn, json!({"enabled": false})).await;
    let inherited = update(&conn, json!({"remove": true})).await;
    assert_eq!(inherited["providers"]["openai"]["enabled"], true);
    assert_eq!(inherited["providers"]["openai"]["apiKeyConfigured"], true);
    assert!(!inherited.to_string().contains("inherited-provider-value"));
    let local = std::fs::read(config_dir.join("settings.json")).unwrap();
    let local: serde_json::Value = serde_json::from_slice(&local).unwrap();
    assert!(local["providers"].get("openai").is_none());
}
