//! ACP authenticate, session/close, and removed-method integration tests.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

#[tokio::test]
async fn test_authenticate_returns_success() {
    let mut harness = build_acp_harness(vec![]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let resp = harness
        .request("authenticate", json!({"methodId": "none"}))
        .await;

    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_session_close_validates_session_id() {
    let mut harness = build_acp_harness(vec![chunks::text_turn("hello")]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let new_resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let sid = new_resp["result"]["sessionId"].as_str().unwrap();

    // Prompt first
    let (resp, _) = harness
        .request_with_notifications(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "hi"}]}),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp);

    // Close with correct sessionId
    let close_resp = harness
        .request("session/close", json!({"sessionId": sid}))
        .await;
    assertions::assert_json_rpc_ok(&close_resp);
}

#[tokio::test]
async fn test_session_close_rejects_wrong_session_id() {
    let mut harness = build_acp_harness(vec![]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    harness.request("session/new", json!({"cwd": "/tmp"})).await;

    // Close with wrong sessionId → error
    let resp = harness
        .request("session/close", json!({"sessionId": "wrong-id"}))
        .await;
    assert!(
        resp.get("error").is_some(),
        "expected error for wrong sessionId: {resp}"
    );
}

#[tokio::test]
async fn test_session_list_returns_sessions() {
    let mut harness = build_acp_harness(vec![]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let resp = harness.request("session/list", json!({})).await;

    // Before creating a session, list should return empty
    let result = &resp["result"];
    assert!(
        result.get("sessions").is_some(),
        "expected sessions field: {resp}"
    );
}

#[tokio::test]
async fn test_session_load_returns_not_supported() {
    // session/load is not implemented (agent/join not available).
    let mut harness = build_acp_harness(vec![]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let resp = harness
        .request(
            "session/load",
            json!({"sessionId": "s", "cwd": "/tmp", "mcpServers": []}),
        )
        .await;

    let error = &resp["error"];
    assert!(error.is_object(), "expected error: {resp}");
    assert!(
        error["message"]
            .as_str()
            .unwrap_or("")
            .contains("not supported"),
        "expected 'not supported' message: {error}"
    );
}
