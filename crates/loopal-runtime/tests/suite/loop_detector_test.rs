use loopal_protocol::{InterruptSignal, MessageSource};
use loopal_provider_api::ContentBlock;
use loopal_runtime::agent_loop::cancel::TurnCancel;
use loopal_runtime::agent_loop::governance::{Governance, TurnHook, Verdict};
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

fn tool(name: &str) -> (String, String, serde_json::Value) {
    ("id".into(), name.into(), json!({"file": "/tmp/x.rs"}))
}

fn result(content: &str) -> Vec<ContentBlock> {
    vec![ContentBlock::ToolResult {
        tool_use_id: "id".into(),
        content: content.into(),
        images: vec![],
        is_error: false,
        metadata: None,
    }]
}

// One before→execute→after cycle; returns the pre-execution verdict and feeds
// `content` as this call's output so the detector can track output stability.
fn cycle(det: &mut LoopDetector, ctx: &mut TurnContext, name: &str, content: &str) -> Verdict {
    let calls = [tool(name)];
    let v = det.on_before_tools(ctx, &calls);
    det.on_after_tools(ctx, &calls, &result(content));
    v
}

// --- trait defaults ---

#[test]
fn governance_defaults_are_continue() {
    struct NoopGovernance;
    impl Governance for NoopGovernance {}
    let mut g = NoopGovernance;
    let mut ctx = make_ctx();
    assert!(matches!(
        g.on_before_tools(&mut ctx, &[tool("Read")]),
        Verdict::Continue
    ));
    g.on_after_tools(&mut ctx, &[tool("Read")], &[]);
    g.on_envelope_received(&MessageSource::Human);
}

#[test]
fn turn_hook_defaults_are_noop() {
    struct NoopHook;
    impl TurnHook for NoopHook {}
    let mut h = NoopHook;
    let mut ctx = make_ctx();
    h.on_turn_start(&mut ctx);
    h.on_after_tools(&mut ctx, &[tool("Read")], &[]);
    h.on_turn_end(&ctx);
}

// --- stationary repetition (same input → same output) trips the detector ---

#[test]
fn identical_output_three_times_warns() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    for _ in 0..3 {
        cycle(&mut det, &mut ctx, "Read", "same");
    }
    assert!(matches!(
        cycle(&mut det, &mut ctx, "Read", "same"),
        Verdict::InjectWarning(_)
    ));
}

#[test]
fn identical_output_five_times_aborts() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    for _ in 0..5 {
        cycle(&mut det, &mut ctx, "Read", "same");
    }
    let Verdict::AbortTurn {
        reason,
        feedback_to_model,
    } = cycle(&mut det, &mut ctx, "Read", "same")
    else {
        panic!("expected AbortTurn after 5 identical outputs");
    };
    assert!(reason.contains("Loop detected"));
    assert!(feedback_to_model.contains("Read"));
}

// --- the ReadImage regression: same args, fresh output each call ---

#[test]
fn changing_output_never_aborts() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    // Same tool + same args (e.g. ReadImage on an overwritten screenshot path)
    // but a different result every call — must never be flagged.
    for i in 0..8 {
        let v = cycle(&mut det, &mut ctx, "ReadImage", &format!("frame-{i}"));
        assert!(
            matches!(v, Verdict::Continue),
            "fresh output must reset the streak at iteration {i}, got {v:?}"
        );
    }
}

#[test]
fn single_cycle_returns_continue() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    assert!(matches!(
        cycle(&mut det, &mut ctx, "Read", "x"),
        Verdict::Continue
    ));
}

// --- resets ---

fn prime_to_abort(det: &mut LoopDetector, ctx: &mut TurnContext) {
    for _ in 0..5 {
        cycle(det, ctx, "Read", "same");
    }
}

#[test]
fn human_envelope_resets() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    prime_to_abort(&mut det, &mut ctx);
    det.on_envelope_received(&MessageSource::Human);
    assert!(matches!(
        cycle(&mut det, &mut ctx, "Read", "same"),
        Verdict::Continue
    ));
}

#[test]
fn scheduled_envelope_resets() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    prime_to_abort(&mut det, &mut ctx);
    det.on_envelope_received(&MessageSource::Scheduled);
    assert!(matches!(
        cycle(&mut det, &mut ctx, "Read", "same"),
        Verdict::Continue
    ));
}

#[test]
fn system_envelope_does_not_reset() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    prime_to_abort(&mut det, &mut ctx);
    det.on_envelope_received(&MessageSource::System("goal_continuation".into()));
    assert!(matches!(
        cycle(&mut det, &mut ctx, "Read", "same"),
        Verdict::AbortTurn { .. }
    ));
}

#[test]
fn compact_completed_resets() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    prime_to_abort(&mut det, &mut ctx);
    det.on_compact_completed();
    assert!(matches!(
        cycle(&mut det, &mut ctx, "Read", "same"),
        Verdict::Continue
    ));
}

#[test]
fn different_tools_are_independent() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    cycle(&mut det, &mut ctx, "Read", "a");
    cycle(&mut det, &mut ctx, "Write", "a");
    cycle(&mut det, &mut ctx, "Read", "a");
    assert!(matches!(
        cycle(&mut det, &mut ctx, "Write", "a"),
        Verdict::Continue
    ));
}
