use std::time::Duration;

use loopal_tool_api::Tool;
use serde_json::json;

use crate::test_support::{extract_pid, make_bash, make_ctx, make_store};

#[tokio::test]
async fn output_returns_error_for_unknown_id() {
    let store = make_store();
    let output = loopal_tool_background::ops::bg_output(
        &store,
        "bg_nonexistent_99999",
        true,
        Duration::from_secs(1),
    )
    .await;
    assert!(output.is_error);
    assert!(output.content.contains("not found"));
}

#[tokio::test]
async fn stop_returns_error_for_unknown_id() {
    let store = make_store();
    let result = loopal_tool_background::ops::bg_stop(&store, "bg_nonexistent_99999").await;
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn non_blocking_output_returns_status_running_for_long_command() {
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx();

    let result = bash
        .execute(
            json!({"command": "sleep 300", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = extract_pid(&result.content);

    let output =
        loopal_tool_background::ops::bg_output(&store, &pid, false, Duration::from_secs(1)).await;
    assert!(output.content.contains("[Status: Running]"));

    let _ = loopal_tool_background::ops::bg_stop(&store, &pid).await;
}

#[tokio::test]
#[cfg(not(windows))]
async fn blocking_output_times_out_while_command_runs() {
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx();

    let result = bash
        .execute(
            json!({"command": "sleep 300", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = extract_pid(&result.content);

    let output =
        loopal_tool_background::ops::bg_output(&store, &pid, true, Duration::from_secs(1)).await;
    assert!(output.content.contains("timed out"));

    let _ = loopal_tool_background::ops::bg_stop(&store, &pid).await;
}
