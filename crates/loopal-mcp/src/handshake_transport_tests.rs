use std::time::Duration;

use loopal_secret_client::SecretString;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::HandshakePolicy;
use crate::client::McpClient;

async fn connect_with(
    response: serde_json::Value,
    policy: HandshakePolicy,
) -> Result<McpClient, loopal_error::McpError> {
    connect_with_prefix(Vec::new(), response, policy).await
}

async fn connect_with_prefix(
    prefix: Vec<serde_json::Value>,
    mut response: serde_json::Value,
    policy: HandshakePolicy,
) -> Result<McpClient, loopal_error::McpError> {
    let (client, server) = tokio::io::duplex(16_384);
    tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(server);
        let mut lines = BufReader::new(read).lines();
        let request: serde_json::Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        if response.get("id") == Some(&serde_json::Value::String("$REQUEST_ID".into())) {
            response["id"] = request["id"].clone();
        }
        for message in prefix {
            write
                .write_all(format!("{message}\n").as_bytes())
                .await
                .unwrap();
        }
        write
            .write_all(format!("{response}\n").as_bytes())
            .await
            .unwrap();
        let _ = lines.next_line().await;
    });
    McpClient::connect_with_policy(client, Duration::from_secs(2), None, policy).await
}

fn initialize(version: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "$REQUEST_ID",
        "result": {
            "protocolVersion": version,
            "capabilities": {
                "experimental": {"secret": {"value": "exact-plaintext"}},
                "extensions": {"secret/ext": {"value": "exact-plaintext"}},
                "logging": {"value": "exact-plaintext"},
                "completions": {"value": "exact-plaintext"},
                "prompts": {"listChanged": true},
                "resources": {"subscribe": true, "listChanged": true},
                "tools": {"listChanged": true},
                "tasks": {"list": {"value": "exact-plaintext"}}
            },
            "serverInfo": {
                "name": "server-exact-plaintext",
                "title": "title-exact-plaintext",
                "version": "v-exact-plaintext",
                "description": "description-exact-plaintext",
                "icons": [{"src": "data:text/plain,exact-plaintext"}],
                "websiteUrl": "https://exact-plaintext.test"
            },
            "instructions": "use exact-plaintext"
        }
    })
}

#[tokio::test]
async fn redacts_before_rmcp_logs_or_retains_peer_info() {
    let client = connect_with(
        initialize("2025-03-26"),
        HandshakePolicy::from_seed(&[("token".into(), SecretString::from("exact-plaintext"))]),
    )
    .await
    .unwrap();
    let info = client.peer_info().unwrap();
    let retained = serde_json::to_string(info).unwrap();

    assert!(!retained.contains("exact-plaintext"));
    assert!(retained.contains("<secret_ref:token>"));
    assert!(info.capabilities.tools.is_some());
    assert!(info.capabilities.resources.is_some());
    assert!(info.capabilities.prompts.is_some());
    assert!(info.capabilities.experimental.is_none());
    assert!(info.capabilities.extensions.is_none());
    assert!(info.capabilities.tasks.is_none());
    assert_eq!(info.capabilities.logging.as_ref().unwrap().len(), 0);
    assert!(info.server_info.icons.is_none());
    assert!(info.server_info.website_url.is_none());
}

#[tokio::test]
async fn default_policy_strips_unseeded_handshake_text() {
    let client = connect_with(initialize("2025-06-18"), HandshakePolicy::Strip)
        .await
        .unwrap();
    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.name, "MCP server");
    assert_eq!(info.server_info.version, "");
    assert!(info.instructions.is_none());
    assert!(
        !serde_json::to_string(info)
            .unwrap()
            .contains("exact-plaintext")
    );
}

#[tokio::test]
async fn pre_handshake_notifications_are_discarded_before_the_client_observes_them() {
    let notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/message",
        "params": {"level": "info", "data": "exact-plaintext"}
    });
    let client = connect_with_prefix(
        vec![notification],
        initialize("2025-03-26"),
        HandshakePolicy::Strip,
    )
    .await
    .unwrap();

    assert!(
        !serde_json::to_string(client.peer_info().unwrap())
            .unwrap()
            .contains("exact-plaintext")
    );
}

#[tokio::test]
async fn unknown_version_and_non_initialize_response_fail_closed() {
    for response in [
        initialize("exact-plaintext"),
        serde_json::json!({
            "jsonrpc": "2.0", "id": "$REQUEST_ID", "result": {}
        }),
    ] {
        let error = match connect_with(response, HandshakePolicy::Strip).await {
            Ok(_) => panic!("unsafe handshake accepted"),
            Err(error) => error,
        };
        assert!(!format!("{error}").contains("exact-plaintext"));
    }
}

#[tokio::test]
async fn handshake_errors_drop_server_message_and_data() {
    let error = match connect_with(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "$REQUEST_ID",
            "error": {"code": -32000, "message": "401 exact-plaintext", "data": "exact-plaintext"}
        }),
        HandshakePolicy::Strip,
    )
    .await
    {
        Ok(_) => panic!("unsafe handshake error accepted"),
        Err(error) => error,
    };
    assert!(format!("{error}").contains("authentication required"));
    assert!(!format!("{error:?}").contains("exact-plaintext"));
}

#[tokio::test]
async fn null_error_id_and_wrong_response_id_fail_closed() {
    for response in [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": {
                "code": -32000,
                "message": "authentication exact-plaintext",
                "data": "exact-plaintext"
            }
        }),
        {
            let mut response = initialize("2025-03-26");
            response["id"] = serde_json::json!(999);
            response
        },
    ] {
        let error = match connect_with(response, HandshakePolicy::Strip).await {
            Ok(_) => panic!("mismatched handshake response accepted"),
            Err(error) => error,
        };
        assert!(!format!("{error:?}").contains("exact-plaintext"));
    }
}
