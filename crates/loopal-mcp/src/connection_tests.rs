use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::{McpServerConfig, McpSharing};

use super::McpConnection;
use crate::oauth_credential_seed::{OAUTH_CREDENTIAL_ERROR, OAuthCredentialSeed};
use crate::types::ConnectionStatus;

fn stdio() -> McpServerConfig {
    McpServerConfig::Stdio {
        command: "fixture".into(),
        args: vec!["arg".into()],
        env: HashMap::from([("TOKEN".into(), "{{secret:token}}".into())]),
        enabled: true,
        timeout_ms: 10,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

#[tokio::test]
async fn http_connect_redacts_handshake_instructions_before_caching() {
    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "$REQUEST_ID",
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": {
                "experimental": {"secret": {"value": "exact-plaintext"}}
            },
            "serverInfo": {
                "name": "fixture-exact-plaintext",
                "title": "title exact-plaintext",
                "version": "v-exact-plaintext",
                "description": "description exact-plaintext",
                "icons": [{"src": "data:text/plain,exact-plaintext"}],
                "websiteUrl": "https://exact-plaintext.test"
            },
            "instructions": "use exact-plaintext carefully"
        }
    })
    .to_string()
    .replace("\"$REQUEST_ID\"", "$REQUEST_ID");
    let (url, requests) = crate::http_test_support::server(vec![
        crate::http_test_support::response("200 OK", "application/json", &initialize),
        crate::http_test_support::response("202 Accepted", "application/json", ""),
    ])
    .await;
    let config = McpServerConfig::StreamableHttp {
        url,
        headers: HashMap::from([("x-loopal-token".into(), "{{secret:token}}".into())]),
        enabled: true,
        timeout_ms: 5_000,
        sharing: McpSharing::HubSingleton,
    };
    let mut connection = McpConnection::new("fixture".into(), config, None)
        .with_secret_client(Some(Arc::new(crate::provider_call_tests::SeedClient)));

    connection.connect().await;

    assert!(
        connection.status.is_connected(),
        "errors={:?}, requests={:?}",
        connection.errors,
        requests
            .lock()
            .await
            .iter()
            .map(|request| (&request.method, &request.token))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        connection.instructions.as_deref(),
        Some("use <secret_ref:token> carefully")
    );
    let peer_info = connection.client().unwrap().peer_info().unwrap();
    let retained = serde_json::to_string(peer_info).unwrap();
    assert!(!retained.contains("exact-plaintext"));
    assert!(retained.contains("<secret_ref:token>"));
    assert!(peer_info.capabilities.experimental.is_none());
    assert!(peer_info.server_info.icons.is_none());
    assert!(peer_info.server_info.website_url.is_none());
    let McpServerConfig::StreamableHttp { headers, .. } = &connection.config else {
        unreachable!()
    };
    assert_eq!(headers["x-loopal-token"], "{{secret:token}}");
    assert!(
        requests
            .lock()
            .await
            .iter()
            .all(|request| request.token.as_deref() == Some("exact-plaintext"))
    );
}

#[tokio::test]
async fn disconnect_clears_discovered_state_without_expanding_config() {
    let mut connection = McpConnection::new("fixture".into(), stdio(), None);
    connection.status = ConnectionStatus::Connected;
    connection.instructions = Some("instructions".into());
    connection
        .cached_tools
        .push(loopal_tool_api::ToolDefinition {
            name: "lookup".into(),
            description: String::new(),
            input_schema: serde_json::Value::Null,
        });

    connection.disconnect().await;

    assert_eq!(connection.status, ConnectionStatus::Disconnected);
    assert!(connection.cached_tools.is_empty());
    assert!(connection.instructions.is_none());
    let McpServerConfig::Stdio { env, .. } = &connection.config else {
        unreachable!()
    };
    assert_eq!(env["TOKEN"], "{{secret:token}}");
}

#[tokio::test]
async fn disconnect_closes_and_deactivates_oauth_credential_seed() {
    let client =
        crate::provider_call_tests::fixture_client(serde_json::json!({}), HashMap::new()).await;
    let credentials = Arc::new(OAuthCredentialSeed::default());
    credentials.observe(Some("oauth-access-token")).unwrap();
    let client = client.with_oauth_credentials(credentials.clone());
    let mut connection = McpConnection::new("fixture".into(), stdio(), None)
        .with_secret_client(Some(Arc::new(crate::provider_call_tests::SeedClient)))
        .with_client(client);

    assert_eq!(
        connection
            .result_sanitizer()
            .await
            .unwrap()
            .sanitize_text("oauth-access-token"),
        "<secret_ref:mcp_oauth_access_token>"
    );
    connection.disconnect().await;

    assert_eq!(
        credentials.observe(Some("late-token")),
        Err(OAUTH_CREDENTIAL_ERROR)
    );
}
