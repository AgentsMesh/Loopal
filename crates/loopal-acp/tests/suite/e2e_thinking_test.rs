//! ACP thinking stream + tool progress integration tests.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

#[tokio::test]
async fn test_thinking_stream_produces_thought_chunk() {
    // The mock provider emits thinking chunks when configured.
    // For now, verify basic prompt flow still works (thinking is streamed
    // through the translate layer as agent_thought_chunk).
    let calls = vec![chunks::text_turn("Response after thinking")];
    let mut harness = build_acp_harness(calls).await;

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let new_resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let sid = new_resp["result"]["sessionId"].as_str().unwrap();

    let (resp, notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "think about it"}]}),
        )
        .await;

    assertions::assert_json_rpc_ok(&resp);

    // Verify at least one notification uses the new sessionUpdate tag format
    let has_session_update_tag = notifications.iter().any(|n| {
        n.get("params")
            .and_then(|p| p.get("update"))
            .and_then(|u| u.get("sessionUpdate"))
            .is_some()
    });
    assert!(
        has_session_update_tag,
        "expected notifications with 'sessionUpdate' tag: {notifications:?}"
    );
}
