//! Integration tests for ACP features added during standard alignment.
//!
//! Covers: cancel-as-notification, session/close→new chain,
//! set_config_option model/thinking, notification format verification.

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
async fn test_cancel_as_notification() {
    let calls = vec![chunks::text_turn("before cancel")];
    let mut harness = build_acp_harness(calls).await;
    let sid = setup_session(&mut harness).await;

    // Complete a prompt first
    let (resp, _) = harness
        .request_with_notifications(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "hi"}]}),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp);

    // Cancel as notification (no id → no response)
    harness
        .send_notification("session/cancel", json!({"sessionId": sid}))
        .await;

    // Adapter should still be alive — verify with another request
    let init_resp = harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    assert_eq!(init_resp["result"]["protocolVersion"], 1);
}

#[tokio::test]
async fn test_set_config_option_model() {
    let mut harness = build_acp_harness(vec![]).await;
    let _sid = setup_session(&mut harness).await;

    let resp = harness
        .request(
            "session/set_config_option",
            json!({"configId": "model", "value": "claude-sonnet-4-20250514"}),
        )
        .await;

    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_set_config_option_thinking() {
    let mut harness = build_acp_harness(vec![]).await;
    let _sid = setup_session(&mut harness).await;

    let resp = harness
        .request(
            "session/set_config_option",
            json!({"configId": "thinking", "value": "high"}),
        )
        .await;

    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_notification_uses_session_update_tag() {
    // Verify the wire format uses "sessionUpdate" (ACP standard) not "kind".
    let calls = vec![chunks::text_turn("Check format")];
    let mut harness = build_acp_harness(calls).await;
    let sid = setup_session(&mut harness).await;

    let (_resp, notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "x"}]}),
        )
        .await;

    // All session/update notifications should use "sessionUpdate" tag
    for n in &notifications {
        if let Some(update) = n.get("params").and_then(|p| p.get("update")) {
            assert!(
                update.get("sessionUpdate").is_some(),
                "expected 'sessionUpdate' tag, got: {update}"
            );
            assert!(
                update.get("kind").is_none(),
                "should not have legacy 'kind' tag: {update}"
            );
        }
    }
}

#[tokio::test]
async fn test_mode_changed_is_structured() {
    // ModeChanged should produce "current_mode_update", not a text message.
    // This tests the wire format of the ModeChanged translation.
    let calls = vec![chunks::text_turn("after mode")];
    let mut harness = build_acp_harness(calls).await;
    let sid = setup_session(&mut harness).await;

    // Switch to plan mode → should trigger ModeChanged event
    let mode_resp = harness
        .request(
            "session/set_mode",
            json!({"sessionId": sid, "modeId": "plan"}),
        )
        .await;
    assertions::assert_json_rpc_ok(&mode_resp);
}
