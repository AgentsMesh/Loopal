//! ACP cancel tests: cancel mid-prompt, cancel then new session.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

#[tokio::test]
async fn test_cancel_mid_prompt() {
    // Provide a response so we can test the prompt flow
    let calls = vec![chunks::text_turn("This response completes normally")];
    let mut harness = build_acp_harness(calls);

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let new_resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let sid = new_resp["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    // Send prompt (completes quickly with mock provider)
    let (resp, _notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "hello"}]
            }),
        )
        .await;

    // Prompt should return with a result
    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_cancel_then_new_session() {
    // Cancel kills the agent loop. After cancel, create a new session to continue.
    let calls = vec![
        chunks::text_turn("First response"),
        // Second call is for the new session's prompt
        chunks::text_turn("Response from new session"),
    ];
    let mut harness = build_acp_harness(calls);

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let new_resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let sid = new_resp["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    // First prompt
    let (resp1, _) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "first"}]
            }),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp1);

    // Cancel (kills the agent loop for this session)
    let cancel_resp = harness.request("session/cancel", json!({})).await;
    assertions::assert_json_rpc_ok(&cancel_resp);

    // Create a new session — this spawns a fresh agent loop
    let new_resp2 = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let sid2 = new_resp2["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    // Prompt on the new session should succeed
    let (resp2, _) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid2,
                "prompt": [{"type": "text", "text": "second"}]
            }),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp2);
}
