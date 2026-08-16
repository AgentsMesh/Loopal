use super::*;

#[tokio::test]
async fn wait_and_cancel_delegate_errors_to_the_tool_result() {
    let client = Arc::new(RecordingClient {
        calls: Mutex::new(Vec::new()),
    });
    let ctx = context(Some(client.clone()));

    let wait_result = wait::WorkflowWaitTool
        .execute(
            serde_json::json!({
                "request_id": "wreq_wait",
                "run_id": "wrun_known",
                "after_revision": 1,
                "timeout_ms": 10
            }),
            &ctx,
        )
        .await
        .unwrap();
    let cancel_result = cancel::WorkflowCancelTool
        .execute(
            serde_json::json!({
                "request_id": "wreq_cancel",
                "run_id": "wrun_known"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(wait_result.is_error);
    assert!(wait_result.content.contains("wait failed"));
    assert!(cancel_result.is_error);
    assert!(cancel_result.content.contains("cancel failed"));
    assert_eq!(*client.calls.lock().unwrap(), ["wait", "cancel"]);
}
