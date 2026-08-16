use std::time::Duration;

use rmcp::transport::auth::{
    AuthorizationManager, CredentialStore, InMemoryCredentialStore, OAuthTokenResponse,
    StoredCredentials,
};

use super::connect;

async fn authorized(url: &str, token: &str) -> AuthorizationManager {
    let store = InMemoryCredentialStore::new();
    let response: OAuthTokenResponse = serde_json::from_value(serde_json::json!({
        "access_token": token,
        "token_type": "Bearer"
    }))
    .unwrap();
    store
        .save(StoredCredentials::new(
            "loopal-test".into(),
            Some(response),
            Vec::new(),
            None,
        ))
        .await
        .unwrap();
    let mut manager = AuthorizationManager::new(url).await.unwrap();
    manager.set_credential_store(store);
    manager
}

fn initialize(token: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "$REQUEST_ID",
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "serverInfo": {
                "name": format!("server-{token}"),
                "version": token
            },
            "instructions": format!("use {token}")
        }
    })
    .to_string()
    .replace("\"$REQUEST_ID\"", "$REQUEST_ID")
}

#[tokio::test]
async fn authenticated_transport_observes_token_before_initialize_retention() {
    let token = "oauth-transport-access-token";
    let (url, _) = crate::http_test_support::server(vec![
        crate::http_test_support::response("200 OK", "application/json", &initialize(token)),
        crate::http_test_support::response("202 Accepted", "application/json", ""),
    ])
    .await;
    let manager = authorized(&url, token).await;

    let mut client = connect(
        &url,
        reqwest::Client::new(),
        manager,
        Duration::from_secs(2),
        None,
    )
    .await
    .unwrap();

    let peer = serde_json::to_string(client.peer_info().unwrap()).unwrap();
    assert!(!peer.contains(token));
    let credentials = client.oauth_credentials().unwrap();
    assert_eq!(
        credentials.redactor().unwrap().scan_and_redact(token).0,
        "<secret_ref:mcp_oauth_access_token>"
    );
    client.close(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn authenticated_transport_failure_is_fixed_and_closed() {
    let url = "http://127.0.0.1:9/mcp";
    let manager = AuthorizationManager::new(url).await.unwrap();

    let error = match connect(
        url,
        reqwest::Client::new(),
        manager,
        Duration::from_millis(100),
        None,
    )
    .await
    {
        Ok(_) => panic!("unreachable endpoint must fail"),
        Err(error) => error,
    };

    assert_eq!(
        error.to_string(),
        "Connection failed: OAuth MCP connection failed"
    );
}
