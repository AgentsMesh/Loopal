use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::{OutputTail, Tool, ToolContext, TypedBridge};
use loopal_tool_background::ops::bg_output;
use loopal_tool_bash::{BashParams, BashTool};
use serde_json::json;

use crate::test_support::{extract_pid, make_store};

fn make_streaming_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test").with_output_tail(Arc::new(OutputTail::new(20)))
}

#[tokio::test]
#[cfg(not(windows))]
async fn render_preview_includes_stderr_section_when_present() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = TypedBridge::new(BashTool::new(store.clone()));
    let bridge: TypedBridge<BashTool, BashParams> = bash;
    let ctx = make_streaming_ctx(tmp.path());

    let result = bridge
        .execute(
            json!({"command": r#"echo OUT_LINE; echo ERR_LINE >&2"#, "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = extract_pid(&result.content);

    let output = bg_output(&store, &pid, true, Duration::from_secs(3)).await;
    assert!(output.content.contains("OUT_LINE"));
    assert!(output.content.contains("[stdout"));
    assert!(output.content.contains("[stderr"));
    assert!(output.content.contains("ERR_LINE"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn render_preview_marks_stdout_truncated_for_long_output() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = TypedBridge::new(BashTool::new(store.clone()));
    let bridge: TypedBridge<BashTool, BashParams> = bash;
    let ctx = make_streaming_ctx(tmp.path());

    let cmd = r#"for i in $(seq 1 200); do echo "row_$i"; done"#;
    let result = bridge
        .execute(json!({"command": cmd, "run_in_background": true}), &ctx)
        .await
        .unwrap();
    let pid = extract_pid(&result.content);

    let output = bg_output(&store, &pid, true, Duration::from_secs(3)).await;
    assert!(
        output.content.contains("[stdout, truncated]"),
        "expected truncation marker, got: {}",
        output.content
    );
    assert!(output.content.contains("lines elided"));
}
