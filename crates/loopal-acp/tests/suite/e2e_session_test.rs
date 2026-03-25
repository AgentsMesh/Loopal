//! ACP session edge-case integration tests — invalid params, duplicate init, etc.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

#[tokio::test]
async fn test_prompt_missing_session_id() {
    // session/prompt with a completely wrong sessionId → error.
    let mut harness = build_acp_harness(vec![chunks::text_turn("x")]);

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    harness.request("session/new", json!({"cwd": "/tmp"})).await;

    let resp = harness
        .request(
            "session/prompt",
            json!({
                "sessionId": "totally-bogus-id",
                "prompt": [{"type": "text", "text": "hi"}]
            }),
        )
        .await;

    assert!(
        resp.get("error").is_some(),
        "expected error for wrong sessionId: {resp}"
    );
}

#[tokio::test]
async fn test_prompt_invalid_content_format() {
    // Prompt with missing `type` field in content block → deserialization error.
    let mut harness = build_acp_harness(vec![]);

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let new_resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let sid = new_resp["result"]["sessionId"].as_str().unwrap();

    let resp = harness
        .request(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"content": "missing type field"}]
            }),
        )
        .await;

    // Should return INVALID_REQUEST (-32600) due to deserialization failure
    assertions::assert_json_rpc_error(&resp, -32600);
}

#[tokio::test]
async fn test_initialize_twice() {
    // Calling initialize a second time should succeed (idempotent).
    let mut harness = build_acp_harness(vec![]);

    let resp1 = harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    assert_eq!(resp1["result"]["protocolVersion"], 1);

    let resp2 = harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    assert_eq!(resp2["result"]["protocolVersion"], 1);
}

#[tokio::test]
async fn test_session_new_without_cwd() {
    // session/new with empty params — `cwd` should default.
    let mut harness = build_acp_harness(vec![]);

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let resp = harness.request("session/new", json!({})).await;

    assert!(
        resp["result"]["sessionId"].is_string(),
        "expected sessionId even with default cwd: {resp}"
    );
}

#[tokio::test]
async fn test_cancel_without_active_prompt() {
    // Cancel when there IS a session but no active prompt — should still
    // succeed (the token fires but nothing is listening).
    let mut harness = build_acp_harness(vec![]);

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    harness.request("session/new", json!({"cwd": "/tmp"})).await;

    // Cancel immediately (no prompt in flight)
    let resp = harness.request("session/cancel", json!({})).await;

    // Should succeed — cancel token fires harmlessly
    assertions::assert_json_rpc_ok(&resp);
}
