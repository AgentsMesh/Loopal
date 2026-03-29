//! Edge case tests for ACP adapter.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

/// Helper: initialize + create session, return session ID.
async fn setup_session(harness: &mut super::e2e_harness::AcpTestHarness) -> String {
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    resp["result"]["sessionId"]
        .as_str()
        .expect("sessionId")
        .to_string()
}

#[tokio::test]
async fn test_prompt_empty_content_blocks() {
    let mut harness = build_acp_harness(vec![chunks::text_turn("ok")]).await;
    let sid = setup_session(&mut harness).await;

    // Prompt with empty prompt array
    let resp = harness
        .request("session/prompt", json!({"sessionId": sid, "prompt": []}))
        .await;

    // Should succeed with empty text (agent gets empty message)
    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_set_mode_missing_mode_id() {
    let mut harness = build_acp_harness(vec![]).await;
    let _sid = setup_session(&mut harness).await;

    let resp = harness.request("session/set_mode", json!({})).await;
    assert!(
        resp.get("error").is_some(),
        "expected error for missing modeId: {resp}"
    );
}

#[tokio::test]
async fn test_set_config_option_missing_config_id() {
    let mut harness = build_acp_harness(vec![]).await;
    let _sid = setup_session(&mut harness).await;

    let resp = harness
        .request("session/set_config_option", json!({"value": "x"}))
        .await;
    assert!(
        resp.get("error").is_some(),
        "expected error for missing configId: {resp}"
    );
}
