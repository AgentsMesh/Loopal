//! Tests for ToolDispatch: runner-direct tools (AskUser, PlanMode) are
//! intercepted by the runner and never reach Tool::execute().

use std::collections::HashSet;

use loopal_message::ContentBlock;

use super::{make_cancel, make_runner};

/// AskUser is intercepted: produces exactly one ToolResult with the frontend
/// answer, NOT the fallback "(intercepted by runner)" from Tool::execute().
#[tokio::test]
async fn ask_user_intercepted_no_fallback_leak() {
    let (mut runner, _rx) = make_runner();

    let tool_uses = vec![(
        "tc-ask".to_string(),
        "AskUser".to_string(),
        serde_json::json!({
            "questions": [{
                "question": "Pick one",
                "options": [
                    {"label": "A", "description": "Option A"},
                    {"label": "B", "description": "Option B"}
                ]
            }]
        }),
    )];
    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    assert_eq!(runner.params.store.len(), 1);
    let msg = &runner.params.store.messages()[0];
    assert_eq!(msg.content.len(), 1, "expected exactly one ToolResult");

    match &msg.content[0] {
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => {
            assert_eq!(tool_use_id, "tc-ask");
            assert!(!is_error);
            // Must NOT contain the fallback from Tool::execute()
            assert!(
                !content.contains("intercepted by runner"),
                "fallback from execute() leaked: {content}"
            );
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

/// Mixed AskUser + Read: each tool_use_id appears exactly once (no duplicates).
#[tokio::test]
async fn ask_user_plus_read_no_duplicate_tool_result() {
    let (mut runner, _rx) = make_runner();

    let tmp = std::env::temp_dir().join(format!("la_dispatch_{}.txt", std::process::id()));
    std::fs::write(&tmp, "dispatch test").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    let tool_uses = vec![
        (
            "tc-ask".to_string(),
            "AskUser".to_string(),
            serde_json::json!({
                "questions": [{
                    "question": "Pick one",
                    "options": [
                        {"label": "A", "description": "a"},
                        {"label": "B", "description": "b"}
                    ]
                }]
            }),
        ),
        (
            "tc-read".to_string(),
            "Read".to_string(),
            serde_json::json!({"file_path": tmp.to_str().unwrap()}),
        ),
    ];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let msg = &runner.params.store.messages()[0];
    assert_eq!(msg.content.len(), 2, "expected exactly 2 ToolResult blocks");

    // Collect tool_use_ids — must be unique.
    let ids: HashSet<&str> = msg
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 2, "duplicate tool_use_id detected");
    assert!(ids.contains("tc-ask"));
    assert!(ids.contains("tc-read"));

    let _ = std::fs::remove_file(&tmp);
}
