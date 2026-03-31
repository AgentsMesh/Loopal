//! Auto Mode integration tests: classifier allow/deny and fast-path permission.

use super::auto_mode_helpers::*;
use super::make_cancel;

// ── Fast-path: tools that skip classifier ──────────────────────────

/// ReadOnly tool (Read) auto-allowed, no classifier call.
#[tokio::test]
async fn readonly_tool_skips_classifier() {
    let (mut runner, mut event_rx) = make_auto_runner(vec![]);

    let tmp = std::env::temp_dir().join(format!("loopal_auto_ro_{}.txt", std::process::id()));
    std::fs::write(&tmp, "content").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    let tool_uses = vec![(
        "tc-1".into(),
        "Read".into(),
        serde_json::json!({"file_path": tmp.to_str().unwrap()}),
    )];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    assert!(drain_auto_decisions(&mut event_rx).is_empty());
    assert_eq!(runner.params.store.len(), 1);
    let _ = std::fs::remove_file(&tmp);
}

/// Supervised tool (Write) auto-allowed in Auto mode, no classifier call.
#[tokio::test]
async fn supervised_tool_skips_classifier() {
    let (mut runner, mut event_rx) = make_auto_runner(vec![]);

    let tool_uses = vec![(
        "tc-1".into(),
        "Write".into(),
        serde_json::json!({"file_path": "/tmp/test.txt", "content": "hello"}),
    )];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    assert!(
        drain_auto_decisions(&mut event_rx).is_empty(),
        "Supervised should not trigger classification in Auto mode"
    );
}

/// Unknown tool (not registered in Kernel) treated as Allow.
#[tokio::test]
async fn unknown_tool_defaults_to_allow() {
    let (mut runner, mut event_rx) = make_auto_runner(vec![]);

    let tool_uses = vec![(
        "tc-1".into(),
        "NonExistentTool".into(),
        serde_json::json!({}),
    )];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    assert!(drain_auto_decisions(&mut event_rx).is_empty());
}

// ── Classifier path: Dangerous tool routed to LLM ──────────────────

/// Dangerous tool classified as allow → tool executes.
#[tokio::test]
async fn dangerous_tool_classified_allow() {
    let (mut runner, mut event_rx) = make_auto_runner(vec![allow_chunks()]);

    let tool_uses = vec![(
        "tc-1".into(),
        "DangerTool".into(),
        serde_json::json!({"command": "cargo test"}),
    )];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let decisions = drain_auto_decisions(&mut event_rx);
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].0, "DangerTool");
    assert_eq!(decisions[0].1, "allow");

    // Tool was approved → result stored (not an error).
    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        loopal_message::ContentBlock::ToolResult { is_error, .. } => {
            assert!(!is_error, "allowed tool should succeed");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

/// Dangerous tool classified as deny → tool denied with reason.
#[tokio::test]
async fn dangerous_tool_classified_deny() {
    let (mut runner, mut event_rx) = make_auto_runner(vec![deny_chunks()]);

    let tool_uses = vec![(
        "tc-1".into(),
        "DangerTool".into(),
        serde_json::json!({"command": "rm -rf /"}),
    )];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let decisions = drain_auto_decisions(&mut event_rx);
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].1, "deny");

    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        loopal_message::ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error, "denied tool should be an error");
            assert!(content.contains("Auto-denied"), "reason: {content}");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

/// Two Dangerous tools classified in parallel: one allow, one deny.
#[tokio::test]
async fn parallel_classification_mixed_results() {
    let (mut runner, mut event_rx) = make_auto_runner(vec![allow_chunks(), deny_chunks()]);

    let tool_uses = vec![
        (
            "tc-1".into(),
            "DangerTool".into(),
            serde_json::json!({"command": "cargo test"}),
        ),
        (
            "tc-2".into(),
            "DangerTool".into(),
            serde_json::json!({"command": "rm -rf /"}),
        ),
    ];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let decisions = drain_auto_decisions(&mut event_rx);
    assert_eq!(decisions.len(), 2);
    // First tool allowed, second denied (order matches input order).
    assert_eq!(decisions[0].1, "allow");
    assert_eq!(decisions[1].1, "deny");
}
