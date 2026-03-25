//! ACP error-handling integration tests — provider errors, tool errors, malformed input.

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
async fn test_provider_error_in_prompt() {
    // Provider returns error during streaming — agent loop catches it and
    // emits an Error event.  The prompt should still complete gracefully.
    let calls = vec![vec![chunks::provider_error("network failure")]];
    let mut harness = build_acp_harness(calls);
    let sid = setup_session(&mut harness).await;

    let (resp, _notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "hello"}]
            }),
        )
        .await;

    // The server must respond (either success or error), not crash / hang.
    assert!(
        resp.get("result").is_some() || resp.get("error").is_some(),
        "expected a JSON-RPC response: {resp}"
    );
}

#[tokio::test]
async fn test_tool_error_recovers() {
    // Agent calls Read on a nonexistent path → tool returns error result →
    // second turn produces normal text.
    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "Read",
            json!({"file_path": "/nonexistent/file.txt"}),
        ),
        chunks::text_turn("File not found, sorry."),
    ];
    let mut harness = build_acp_harness(calls);
    let sid = setup_session(&mut harness).await;

    let (resp, _notifs) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "read the file"}]
            }),
        )
        .await;

    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_malformed_input_then_valid_request() {
    // Sending non-JSON on the wire should be silently skipped by
    // `read_message`; subsequent valid requests still succeed.
    let mut harness = build_acp_harness(vec![]);

    // Write raw garbage via the duplex stream
    use tokio::io::AsyncWriteExt;
    harness
        .client_writer
        .write_all(b"this is not json\n")
        .await
        .unwrap();
    harness.client_writer.flush().await.unwrap();

    // Next valid request should work
    let resp = harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    assert_eq!(resp["result"]["protocolVersion"], 1);
}
