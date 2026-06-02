use loopal_protocol::InterruptSignal;
use loopal_provider_api::ContentBlock;
use loopal_runtime::agent_loop::cancel::TurnCancel;
use loopal_runtime::agent_loop::governance::{Governance, Verdict};
use loopal_runtime::agent_loop::loop_detector::LoopDetector;
use loopal_runtime::agent_loop::turn_context::TurnContext;
use loopal_tool_invocation::{ToolImageBlock, ToolResultMetadata};
use serde_json::json;
use std::sync::Arc;

fn make_ctx() -> TurnContext {
    let cancel = TurnCancel::new(
        InterruptSignal::new(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    TurnContext::new(0, cancel)
}

fn image_result(data: &str) -> Vec<ContentBlock> {
    vec![ContentBlock::ToolResult {
        tool_use_id: "id".into(),
        content: String::new(),
        images: vec![ToolImageBlock::inline("image/png", data.to_string())],
        is_error: false,
        metadata: None,
    }]
}

#[test]
fn changing_image_content_never_aborts() {
    // Same path (same input), different image bytes each call. content is empty
    // so only the image distinguishes calls — pins #A: the digest must hash
    // image content (data/id), not just byte_size. "frame-N" strings share the
    // same byte_size, so a byte_size-only digest would falsely abort.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [(
        "id".into(),
        "ReadImage".into(),
        json!({"file": "/tmp/c.png"}),
    )];
    for i in 0..8 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &image_result(&format!("frame-{i}")));
    }
    assert!(matches!(
        det.on_before_tools(&mut ctx, &calls),
        Verdict::Continue
    ));
}

#[test]
fn identical_image_content_still_aborts() {
    // Same path AND identical image bytes → genuine loop → must abort.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [(
        "id".into(),
        "ReadImage".into(),
        json!({"file": "/tmp/c.png"}),
    )];
    for _ in 0..5 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &image_result("same-bytes"));
    }
    assert!(matches!(
        det.on_before_tools(&mut ctx, &calls),
        Verdict::AbortTurn { .. }
    ));
}

#[test]
fn absent_result_does_not_accrue() {
    // output_digest_for returns None when no ToolResult matches the id; the
    // streak must not accrue (None must not fold to one shared empty digest).
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [("id".into(), "Read".into(), json!({"file": "/tmp/x.rs"}))];
    let unrelated = vec![ContentBlock::ToolResult {
        tool_use_id: "other".into(),
        content: "x".into(),
        images: vec![],
        is_error: false,
        metadata: None,
    }];
    for _ in 0..7 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &unrelated);
    }
    assert!(matches!(
        det.on_before_tools(&mut ctx, &calls),
        Verdict::Continue
    ));
}

fn write_result(count: u64) -> Vec<ContentBlock> {
    vec![ContentBlock::ToolResult {
        tool_use_id: "id".into(),
        content: "File written".into(),
        images: vec![],
        is_error: false,
        metadata: Some(ToolResultMetadata::bytes_written(count)),
    }]
}

#[test]
fn changing_metadata_never_aborts() {
    // Fixed content + is_error, only the metadata (byte count) changes — must
    // not be flagged as a loop, since the result genuinely differs each call.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [("id".into(), "Write".into(), json!({"file": "/tmp/x"}))];
    for i in 0..8 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &write_result(i));
    }
    assert!(matches!(
        det.on_before_tools(&mut ctx, &calls),
        Verdict::Continue
    ));
}

#[test]
fn on_turn_cancelled_resets_streak() {
    // A user interrupt resets the loop streak. Prove a FULL reset (not a
    // decrement): post-cancel the next identical call is Continue (count back
    // to 0, below warn threshold), AND a fresh run of 5 identical calls accrues
    // from zero to AbortTurn again — a decrement-by-one would leave the streak
    // hot and either warn immediately or abort one call early.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [(
        "id".into(),
        "ReadImage".into(),
        json!({"file": "/tmp/c.png"}),
    )];
    for _ in 0..5 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &image_result("same-bytes"));
    }
    det.on_turn_cancelled();
    assert!(
        matches!(det.on_before_tools(&mut ctx, &calls), Verdict::Continue),
        "first post-cancel call must be Continue (streak cleared, not warning)"
    );
    for _ in 0..5 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &image_result("same-bytes"));
    }
    assert!(
        matches!(
            det.on_before_tools(&mut ctx, &calls),
            Verdict::AbortTurn { .. }
        ),
        "streak must re-accrue from zero after the reset"
    );
}
