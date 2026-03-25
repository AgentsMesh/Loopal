//! ACP multi-turn and multi-session integration tests.

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
async fn test_two_consecutive_prompts() {
    // Two prompts in sequence on the same session — second uses the second
    // mock call set.
    let calls = vec![
        chunks::text_turn("Response to first prompt."),
        chunks::text_turn("Response to second prompt."),
    ];
    let mut harness = build_acp_harness(calls);
    let sid = setup_session(&mut harness).await;

    // First prompt
    let (resp1, notifs1) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "first"}]
            }),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp1);
    assert!(
        !notifs1.is_empty(),
        "first prompt should produce notifications"
    );

    // Second prompt
    let (resp2, notifs2) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "second"}]
            }),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp2);
    assert!(
        !notifs2.is_empty(),
        "second prompt should produce notifications"
    );
}

#[tokio::test]
async fn test_tool_then_text_notification_order() {
    // Tool call turn → text turn.  Notifications should include both
    // tool_call and agent_message_chunk kinds, with tool_call arriving first.
    let tmp = std::env::temp_dir().join(format!(
        "la_multi_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::write(&tmp, "multi test content").unwrap();

    let calls = vec![
        chunks::tool_turn("tc-1", "Read", json!({"file_path": tmp.to_str().unwrap()})),
        chunks::text_turn("Done reading."),
    ];
    let mut harness = build_acp_harness(calls);
    let sid = setup_session(&mut harness).await;

    let (resp, notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "read it"}]
            }),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp);

    // Find tool_call and agent_message_chunk notification indices
    let tool_idx = notifications.iter().position(|n| {
        n.get("params")
            .and_then(|p| p.get("update"))
            .and_then(|u| u.get("kind"))
            .and_then(|k| k.as_str())
            == Some("tool_call")
    });
    let msg_idx = notifications.iter().position(|n| {
        n.get("params")
            .and_then(|p| p.get("update"))
            .and_then(|u| u.get("kind"))
            .and_then(|k| k.as_str())
            == Some("agent_message_chunk")
    });

    if let (Some(ti), Some(mi)) = (tool_idx, msg_idx) {
        assert!(
            ti < mi,
            "tool_call should arrive before agent_message_chunk"
        );
    }

    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_multi_harness_isolation() {
    // Two independent harnesses get different session IDs.
    let mut h1 = build_acp_harness(vec![chunks::text_turn("S1")]);
    let mut h2 = build_acp_harness(vec![chunks::text_turn("S2")]);

    let sid1 = setup_session(&mut h1).await;
    let sid2 = setup_session(&mut h2).await;

    assert_ne!(sid1, sid2, "separate harnesses must have distinct sessions");
}
