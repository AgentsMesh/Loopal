//! ACP protocol integration tests — tool calls, cancel, error handling.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_acp_harness;

#[tokio::test]
async fn test_acp_tool_call_notifications() {
    let tmp = std::env::temp_dir().join(format!(
        "la_acp_read_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::write(&tmp, "acp test content").unwrap();

    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "Read",
            serde_json::json!({"file_path": tmp.to_str().unwrap()}),
        ),
        chunks::text_turn("Read done."),
    ];

    let mut harness = build_acp_harness(calls).await;

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
                "prompt": [{"type": "text", "text": "read the file"}]
            }),
        )
        .await;

    assertions::assert_json_rpc_ok(&resp);
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_acp_no_session_cancel_error() {
    let mut harness = build_acp_harness(vec![]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let resp = harness.request("session/cancel", json!({})).await;

    let error = &resp["error"];
    assert!(
        error.is_object(),
        "expected error for cancel without session"
    );
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("no active session")
    );
}

#[tokio::test]
async fn test_acp_prompt_without_session_error() {
    let mut harness = build_acp_harness(vec![]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;

    let resp = harness
        .request(
            "session/prompt",
            json!({
                "sessionId": "nonexistent",
                "prompt": [{"type": "text", "text": "hello"}]
            }),
        )
        .await;

    assert!(resp.get("error").is_some(), "expected error: {resp}");
}
