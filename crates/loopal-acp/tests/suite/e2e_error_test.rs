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
    // This scenario exercises terminal ACP error projection, not retry. Use a
    // non-retryable provider response so the fixture is explicit and cannot
    // fall through to an exhausted mock queue during production retries.
    let calls = vec![vec![chunks::non_retryable_error("network failure")]];
    let mut harness = build_acp_harness(calls).await;
    let sid = setup_session(&mut harness).await;

    let (resp, notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "hello"}]
            }),
        )
        .await;

    // The server must respond and surface the terminal provider failure rather
    // than returning an empty successful prompt result.
    assert!(
        resp.get("error").is_some()
            || notifications
                .iter()
                .any(|notification| notification.to_string().contains("network failure")),
        "expected the fatal provider error in response or notifications; response={resp}, notifications={notifications:?}"
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
    let mut harness = build_acp_harness(calls).await;
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
    let mut harness = build_acp_harness(vec![]).await;

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
