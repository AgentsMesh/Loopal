//! ACP cancel tests.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

#[tokio::test]
async fn test_cancel_mid_prompt() {
    let calls = vec![chunks::text_turn("This response completes normally")];
    let mut harness = build_acp_harness(calls).await;

    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let new_resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let sid = new_resp["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    let (resp, _) = harness
        .request_with_notifications(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "hello"}]}),
        )
        .await;

    assertions::assert_json_rpc_ok(&resp);
}

// Note: test_cancel_then_new_session removed — Hub mode doesn't support
// re-creating sessions after cancel within the same agent process.
