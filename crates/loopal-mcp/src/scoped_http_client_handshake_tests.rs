use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage, ServerResult};
use rmcp::transport::streamable_http_client::{StreamableHttpClient, StreamableHttpPostResponse};

use super::ScopedHttpClient;
use super::tests::{RotatingClient, config, provenance};
use crate::http_test_support::{response, server};

fn initialize_response() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "$REQUEST_ID",
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": {"experimental": {"secret": {"value": "secret-1"}}},
            "serverInfo": {"name": "server-secret-1", "version": "secret-1"},
            "instructions": "use secret-1"
        }
    })
    .to_string()
    .replace("\"$REQUEST_ID\"", "$REQUEST_ID")
}

fn initialize(id: i64) -> ClientJsonRpcMessage {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "fixture", "version": "1"}
        }
    }))
    .unwrap()
}

#[tokio::test]
async fn every_http_initialize_is_sanitized_before_worker_observation() {
    let (url, requests) = server(vec![
        response("200 OK", "application/json", &initialize_response()),
        response("200 OK", "application/json", &initialize_response()),
    ])
    .await;
    let secrets = Arc::new(RotatingClient {
        calls: AtomicUsize::new(0),
        rotate: false,
    });
    let client = ScopedHttpClient::new(
        config("x-loopal-token", "{{secret:token}}"),
        Some(secrets.clone()),
        provenance(),
    );

    for id in [1, 2] {
        let result = client
            .post_message(
                url.clone().into(),
                initialize(id),
                None,
                None,
                HashMap::new(),
            )
            .await
            .unwrap();
        let StreamableHttpPostResponse::Json(message, _) = result else {
            panic!("initialize response was not JSON")
        };
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(!encoded.contains("secret-1"));
        assert!(encoded.contains("<secret_ref:token>"));
        let ServerJsonRpcMessage::Response(response) = message else {
            unreachable!()
        };
        let ServerResult::InitializeResult(info) = response.result else {
            unreachable!()
        };
        assert!(info.capabilities.experimental.is_none());
    }
    assert_eq!(requests.lock().await.len(), 2);
    assert_eq!(secrets.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn rotation_between_handshake_seed_and_headers_fails_before_network() {
    let (url, requests) = server(Vec::new()).await;
    let client = ScopedHttpClient::new(
        config("x-loopal-token", "{{secret:token}}"),
        Some(Arc::new(RotatingClient {
            calls: AtomicUsize::new(0),
            rotate: true,
        })),
        provenance(),
    );

    let error = client
        .post_message(url.into(), initialize(1), None, None, HashMap::new())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("secret configuration unavailable")
    );
    assert!(requests.lock().await.is_empty());
}
