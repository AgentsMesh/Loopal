use loopal_ipc::protocol::methods;
use serde_json::{Value, json};

use super::workspace_rpc_support::setup;

fn stdio(name: &str, patches: Value) -> Value {
    json!({
        "workspaceId": "local-workspace",
        "server": {
            "type": "stdio", "name": name, "command": "node",
            "args": ["server.js"], "enabled": true, "timeoutMs": 12000,
            "sharing": "per-agent", "cwdIsolation": {
                "arg": "--user-data-dir", "cacheSubdir": "desktop-mcp"
            },
            "secretPatches": patches
        }
    })
}

async fn list(conn: &std::sync::Arc<loopal_ipc::Connection<loopal_ipc::Listening>>) -> Value {
    conn.send_request(
        methods::DESKTOP_LIST_MCP_SERVERS.name,
        json!({"workspaceId": "local-workspace"}),
    )
    .await
    .unwrap()
}

include!("desktop_mcp_settings_rpc_test/lifecycle.rs");
include!("desktop_mcp_settings_rpc_test/validation.rs");
