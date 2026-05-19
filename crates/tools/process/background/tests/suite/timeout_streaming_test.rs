use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::{OutputTail, Tool, ToolContext};
use loopal_tool_background::ops::bg_output;
use serde_json::json;

use crate::test_support::{extract_pid, make_bash, make_store, unique_sid};

fn make_streaming_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        unique_sid(),
    );
    ToolContext::new(backend, "test").with_output_tail(Arc::new(OutputTail::new(20)))
}

#[tokio::test]
#[cfg(not(windows))]
async fn timeout_converted_to_bg_continues_to_stream_output() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_streaming_ctx(tmp.path());

    // reason: write 10 lines spaced 250ms apart (~2.5s total). timeout=1
    // triggers conversion after ~3 lines; we then verify additional lines
    // keep appearing in subsequent bg_output queries — this is the bug fix
    // for the timeout→bg output freeze.
    let cmd = r#"for i in 1 2 3 4 5 6 7 8 9 10; do echo "line_$i"; sleep 0.25; done"#;
    let result = bash
        .execute(json!({"command": cmd, "timeout": 1}), &ctx)
        .await
        .unwrap();
    assert!(
        !result.is_error,
        "streaming timeout should convert to bg, got: {}",
        result.content
    );
    let pid = extract_pid(&result.content);

    let first = bg_output(&store, &pid, false, Duration::from_millis(50)).await;
    assert!(!first.is_error);
    let first_count = first.content.matches("line_").count();
    assert!(
        first_count >= 1,
        "expected lines at conversion, got: {}",
        first.content
    );

    tokio::time::sleep(Duration::from_millis(1800)).await;

    let second = bg_output(&store, &pid, false, Duration::from_millis(50)).await;
    let second_count = second.content.matches("line_").count();
    assert!(
        second_count > first_count,
        "output must grow after timeout conversion (first={first_count}, second={second_count}): {}",
        second.content
    );
}

#[tokio::test]
#[cfg(not(windows))]
async fn timeout_converted_to_bg_reaches_completed_state() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_streaming_ctx(tmp.path());

    let cmd = r#"for i in 1 2 3 4 5; do echo "tick_$i"; sleep 0.4; done"#;
    let result = bash
        .execute(json!({"command": cmd, "timeout": 1}), &ctx)
        .await
        .unwrap();
    let pid = extract_pid(&result.content);

    let final_output = bg_output(&store, &pid, true, Duration::from_secs(5)).await;
    assert!(!final_output.is_error);
    assert!(
        final_output.content.contains("tick_5"),
        "final tick must appear after blocking output, got: {}",
        final_output.content
    );
    assert!(
        final_output.content.contains("Completed") || final_output.content.contains("[Completed"),
        "expected Completed status, got: {}",
        final_output.content
    );
}
