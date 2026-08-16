use std::collections::HashMap;

use futures::StreamExt;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::streamable_http_client::{
    AuthRequiredError, InsufficientScopeError, StreamableHttpClient, StreamableHttpError,
    StreamableHttpPostResponse,
};

use super::HandshakeStrippingHttpClient;
use super::test_support::FakeHttpClient;

fn initialize(id: i64) -> ClientJsonRpcMessage {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0", "id": id, "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26", "capabilities": {},
            "clientInfo": {"name": "fixture", "version": "1"}
        }
    }))
    .unwrap()
}

fn ping(id: i64) -> ClientJsonRpcMessage {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0", "id": id, "method": "ping"
    }))
    .unwrap()
}

fn initialize_result(id: i64) -> ServerJsonRpcMessage {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0", "id": id,
        "result": {
            "protocolVersion": "2025-03-26", "capabilities": {"tools": {}},
            "serverInfo": {"name": "exact-plaintext", "version": "exact-plaintext"},
            "instructions": "exact-plaintext"
        }
    }))
    .unwrap()
}

#[tokio::test]
async fn strips_initialize_json_and_preserves_ordinary_posts() {
    let ordinary = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "result": {}
    }))
    .unwrap();
    let fake = FakeHttpClient::new(vec![
        Ok(StreamableHttpPostResponse::Json(
            initialize_result(1),
            Some("ordinary-session".into()),
        )),
        Ok(StreamableHttpPostResponse::Json(ordinary, None)),
    ]);
    let client = HandshakeStrippingHttpClient::new(fake);

    let initialized = client
        .post_message(
            "http://test".into(),
            initialize(1),
            None,
            None,
            HashMap::new(),
        )
        .await
        .unwrap();
    assert!(!format!("{initialized:?}").contains("exact-plaintext"));
    let ordinary = client
        .post_message("http://test".into(), ping(2), None, None, HashMap::new())
        .await
        .unwrap();
    assert!(matches!(ordinary, StreamableHttpPostResponse::Json(_, _)));
}

#[tokio::test]
async fn sanitizes_handshake_errors_and_delegates_session_methods() {
    let fake = FakeHttpClient::new(vec![Err("ordinary error exact-plaintext".into())]);
    let deletes = fake.deletes.clone();
    let gets = fake.gets.clone();
    let client = HandshakeStrippingHttpClient::new(fake);

    let error = client
        .post_message(
            "http://test".into(),
            initialize(1),
            None,
            None,
            HashMap::new(),
        )
        .await
        .unwrap_err();
    assert!(!error.to_string().contains("exact-plaintext"));
    client
        .delete_session("http://test".into(), "session".into(), None, HashMap::new())
        .await
        .unwrap();
    let _ = client
        .get_stream(
            "http://test".into(),
            "session".into(),
            None,
            None,
            HashMap::new(),
        )
        .await
        .unwrap();
    assert_eq!(*deletes.lock().unwrap(), 1);
    assert_eq!(*gets.lock().unwrap(), 1);

    let auth = super::sanitize_initial_error::<reqwest::Error>(StreamableHttpError::AuthRequired(
        AuthRequiredError::new("exact-plaintext".into()),
    ));
    assert!(auth.to_string().contains("authentication required"));
    assert!(!auth.to_string().contains("exact-plaintext"));

    let insufficient =
        super::sanitize_initial_error::<reqwest::Error>(StreamableHttpError::InsufficientScope(
            InsufficientScopeError::new("exact-plaintext".into(), Some("exact-plaintext".into())),
        ));
    assert!(insufficient.to_string().contains("authentication required"));
    assert!(!insufficient.to_string().contains("exact-plaintext"));
}

#[tokio::test]
async fn rejects_non_handshake_json_for_an_initialize_request() {
    let notification: ServerJsonRpcMessage = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/message",
        "params": {"level": "info", "data": "exact-plaintext"}
    }))
    .unwrap();
    let client = HandshakeStrippingHttpClient::new(FakeHttpClient::new(vec![Ok(
        StreamableHttpPostResponse::Json(notification, None),
    )]));

    let error = client
        .post_message(
            "http://test".into(),
            initialize(1),
            None,
            None,
            HashMap::new(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("sanitization unavailable"));
    assert!(!error.to_string().contains("exact-plaintext"));
}

#[tokio::test]
async fn sanitizes_sse_before_worker_parsing() {
    let body = format!(
        "event: exact-plaintext\nid: exact-plaintext\ndata: not json\n\nevent: exact-plaintext\nid: exact-plaintext\nretry: 99\ndata: {}\n\n",
        serde_json::to_string(&initialize_result(1)).unwrap()
    );
    let (url, _) = crate::http_test_support::server(vec![crate::http_test_support::response(
        "200 OK",
        "text/event-stream",
        &body,
    )])
    .await;
    let client = HandshakeStrippingHttpClient::new(reqwest::Client::new());

    let response = client
        .post_message(url.into(), initialize(1), None, None, HashMap::new())
        .await
        .unwrap();
    let StreamableHttpPostResponse::Sse(mut stream, _) = response else {
        panic!("expected SSE")
    };
    let event = stream.next().await.unwrap().unwrap();
    assert!(event.event.is_none());
    assert!(event.id.is_none());
    assert!(event.retry.is_none());
    assert!(!event.data.unwrap().contains("exact-plaintext"));
    assert!(stream.next().await.is_none());
}
