//! Behavior tests driving the edit-failure hint and the Write staleness guard
//! through the real runner tool pipeline — which is what wires the shared
//! `FileReadTracker` across tool calls in production.

use loopal_provider_api::ContentBlock;
use loopal_tool_api::PermissionMode;
use serde_json::json;

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

#[tokio::test]
async fn edit_no_match_returns_actionable_hint_through_runner() {
    let (mut runner, mut event_rx, _mbox, _ctrl, _perm) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tmp = std::env::temp_dir().join(format!("loopal_edit_hint_{}.txt", std::process::id()));
    std::fs::write(&tmp, "let compute_total = 42;\n").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );

    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![(
            "e1".to_string(),
            "Edit".to_string(),
            json!({
                "file_path": tmp.to_str().unwrap(),
                "old_string": "let compute_total = 99;",
                "new_string": "let compute_total = 7;"
            }),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let view = runner.turns.view();
    match &view.messages()[0].content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error, "a no-match edit should error");
            assert!(
                content.contains("Nearest line") && content.contains("Re-read"),
                "edit failure should carry an actionable hint, got: {content}"
            );
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn write_refuses_clobber_after_read_through_runner() {
    let (mut runner, mut event_rx, _mbox, _ctrl, _perm) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tmp = std::env::temp_dir().join(format!("loopal_stale_{}.txt", std::process::id()));
    std::fs::write(&tmp, "original").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );

    // 1. Model reads the file — the runner records it in the shared read tracker.
    let mut read_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut read_ctx,
        vec![(
            "r1".to_string(),
            "Read".to_string(),
            json!({"file_path": tmp.to_str().unwrap()}),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    // 2. Another editor changes the file on disk.
    std::fs::write(&tmp, "changed by another editor").unwrap();

    // 3. Model tries to overwrite — the guard must refuse, not clobber.
    let mut write_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut write_ctx,
        vec![(
            "w1".to_string(),
            "Write".to_string(),
            json!({"file_path": tmp.to_str().unwrap(), "content": "clobber"}),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let view = runner.turns.view();
    let write_msg = view.messages().last().expect("write-turn message");
    match &write_msg.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error, "a stale overwrite should be refused");
            assert!(content.contains("changed on disk"), "got: {content}");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(&tmp).unwrap(),
        "changed by another editor",
        "the other editor's content must be preserved, not clobbered"
    );
    let _ = std::fs::remove_file(&tmp);
}
