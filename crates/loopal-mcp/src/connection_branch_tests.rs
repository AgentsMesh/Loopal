use std::collections::HashMap;

use loopal_config::{McpServerConfig, McpSharing};

use super::McpConnection;

#[tokio::test]
async fn connect_timeout_surfaces_only_the_fixed_connection_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (_socket, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let config = McpServerConfig::StreamableHttp {
        url,
        headers: HashMap::new(),
        enabled: true,
        timeout_ms: 20,
        sharing: McpSharing::HubSingleton,
    };
    let mut connection = McpConnection::new("timeout-fixture".into(), config, None);

    connection.connect().await;
    server.abort();

    assert!(connection.status.is_failed());
    assert_eq!(
        connection.errors,
        ["connection failed: MCP connection failed"]
    );
}

#[tokio::test]
async fn discovery_failure_keeps_the_transport_connected_with_safe_diagnostics() {
    let initialize = serde_json::json!({
        "jsonrpc": "2.0", "id": "$REQUEST_ID",
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fixture", "version": "1"}
        }
    })
    .to_string()
    .replace("\"$REQUEST_ID\"", "$REQUEST_ID");
    let list_error = serde_json::json!({
        "jsonrpc": "2.0", "id": "$REQUEST_ID",
        "error": {"code": -32000, "message": "exact-plaintext"}
    })
    .to_string()
    .replace("\"$REQUEST_ID\"", "$REQUEST_ID");
    let (url, _) = crate::http_test_support::server(vec![
        crate::http_test_support::response("200 OK", "application/json", &initialize),
        crate::http_test_support::response("202 Accepted", "application/json", ""),
        crate::http_test_support::response("200 OK", "application/json", &list_error),
    ])
    .await;
    let config = McpServerConfig::StreamableHttp {
        url,
        headers: HashMap::new(),
        enabled: true,
        timeout_ms: 2_000,
        sharing: McpSharing::HubSingleton,
    };
    let mut connection = McpConnection::new("fixture".into(), config, None);

    connection.connect().await;

    assert!(connection.status.is_connected());
    assert_eq!(connection.errors, ["tools/list failed"]);
    assert!(!format!("{:?}", connection.errors).contains("exact-plaintext"));
}
