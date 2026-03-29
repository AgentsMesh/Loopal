//! ACP lifecycle integration tests — initialize, session, prompt flow.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

#[tokio::test]
async fn test_acp_initialize() {
    let mut harness = build_acp_harness(vec![]).await;

    let resp = harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let result = &resp["result"];
    assert_eq!(result["protocolVersion"], 1);
    assert_eq!(result["agentInfo"]["name"], "loopal");
}

#[tokio::test]
async fn test_acp_session_new() {
    let mut harness = build_acp_harness(vec![]).await;

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;

    assert!(
        resp["result"]["sessionId"].is_string(),
        "expected sessionId: {resp}"
    );
}

#[tokio::test]
async fn test_acp_full_prompt_lifecycle() {
    let mut harness = build_acp_harness(vec![chunks::text_turn("Hello from ACP!")]).await;

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let new_resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let session_id = new_resp["result"]["sessionId"].as_str().unwrap();

    let (resp, _notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": session_id,
                "prompt": [{"type": "text", "text": "hello"}]
            }),
        )
        .await;

    assertions::assert_json_rpc_ok(&resp);

    // Verify streaming notifications were captured (bootstrap drain fix ensures
    // the event loop processes real events, not stale startup events).
    let has_content = _notifications.iter().any(|n| {
        n.get("params")
            .and_then(|p| p.get("update"))
            .and_then(|u| u.get("content"))
            .is_some()
    });
    assert!(
        has_content,
        "expected session/update notifications with content: {_notifications:?}"
    );
}

#[tokio::test]
async fn test_acp_unknown_method_error() {
    let mut harness = build_acp_harness(vec![]).await;
    let resp = harness.request("nonexistent/method", json!({})).await;
    assertions::assert_json_rpc_error(&resp, -32601);
}
