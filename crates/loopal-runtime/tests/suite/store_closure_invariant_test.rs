//! Sentinel for invariant 5 (消费者视图一致性).
//!
//! After PR 1 (abort path writes compensation tool_results), the in-memory
//! store should be self-closed: every `tool_use` in a stored Assistant
//! message must have a corresponding `tool_result` in a User message after it.
//! `prepare_for_llm` no longer runs `sanitize_tool_pairs` — so the store
//! must produce a closed view without any post-processing.

use loopal_message::{ContentBlock, MessageRole};
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_loop::cancel::TurnCancel;
use loopal_runtime::agent_loop::governance::synthesize_aborted_tool_results;
use loopal_runtime::agent_loop::governance::{Governance, Verdict};
use loopal_runtime::agent_loop::loop_detector::LoopDetector;
use loopal_runtime::agent_loop::turn_context::TurnContext;
use serde_json::json;
use std::sync::Arc;

fn make_ctx() -> TurnContext {
    let cancel = TurnCancel::new(
        InterruptSignal::new(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    TurnContext::new(0, cancel)
}

fn tool(id: &str, name: &str) -> (String, String, serde_json::Value) {
    (id.into(), name.into(), json!({"cmd": "date -u"}))
}

#[test]
fn loop_abort_compensation_produces_closed_pair_message() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();

    let uses = vec![tool("toolu_1", "Bash")];

    // Drive the detector to abort threshold.
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &uses);
    }
    let action = det.on_before_tools(&mut ctx, &uses);
    let (reason, feedback) = match action {
        Verdict::AbortTurn {
            reason,
            feedback_to_model,
        } => (reason, feedback_to_model),
        other => panic!("expected AbortTurn, got {other:?}"),
    };
    assert!(reason.contains("Loop detected"));
    assert!(
        !feedback.is_empty(),
        "Verdict invariant: AbortTurn must carry non-empty feedback_to_model"
    );

    let msg =
        synthesize_aborted_tool_results(&uses, &reason).expect("compensation message must exist");
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content.len(), 1);
    let ContentBlock::ToolResult {
        tool_use_id,
        is_error,
        ..
    } = &msg.content[0]
    else {
        panic!("expected ToolResult block in compensation");
    };
    assert_eq!(tool_use_id, "toolu_1");
    assert!(is_error, "compensation tool_result must be is_error=true");
}

#[test]
fn compensation_covers_every_tool_use_id() {
    let uses = vec![
        tool("toolu_a", "Bash"),
        tool("toolu_b", "Read"),
        tool("toolu_c", "Edit"),
    ];
    let msg = synthesize_aborted_tool_results(&uses, "loop detected").unwrap();
    assert_eq!(msg.content.len(), 3);

    let ids: Vec<&str> = msg
        .content
        .iter()
        .map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => tool_use_id.as_str(),
            _ => panic!("expected ToolResult"),
        })
        .collect();
    assert_eq!(ids, vec!["toolu_a", "toolu_b", "toolu_c"]);
}
