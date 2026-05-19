use loopal_protocol::{InterruptSignal, MessageSource};
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

// --- Governance trait defaults ---

#[test]
fn governance_defaults_are_continue() {
    struct NoopGovernance;
    impl Governance for NoopGovernance {}

    let mut g = NoopGovernance;
    let mut ctx = make_ctx();
    let action = g.on_before_tools(&mut ctx, &[tool("Read")]);
    assert!(matches!(action, Verdict::Continue));
    g.on_envelope_received(&MessageSource::Human);
}

// --- TurnHook trait defaults ---

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

// --- LoopDetector direct tests ---

#[test]
fn loop_detector_no_repeat_returns_continue() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let action = det.on_before_tools(&mut ctx, &[tool("Read")]);
    assert!(matches!(action, Verdict::Continue));
}

#[test]
fn loop_detector_three_repeats_warns() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Read")];
    det.on_before_tools(&mut ctx, &calls);
    det.on_before_tools(&mut ctx, &calls);
    let action = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(action, Verdict::InjectWarning(_)),
        "expected InjectWarning after 3 repeats, got {action:?}"
    );
}

#[test]
fn loop_detector_five_repeats_aborts() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Read")];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
    }
    let action = det.on_before_tools(&mut ctx, &calls);
    let Verdict::AbortTurn {
        reason,
        feedback_to_model,
    } = action
    else {
        panic!("expected AbortTurn after 5 repeats, got {action:?}");
    };
    assert!(reason.contains("Loop detected"));
    assert!(
        !feedback_to_model.is_empty(),
        "AbortTurn must carry a non-empty feedback_to_model so the model sees why"
    );
    assert!(
        feedback_to_model.contains("Read"),
        "feedback_to_model should mention the offending tool name"
    );
}

#[test]
fn loop_detector_human_envelope_resets() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Read")];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
    }
    det.on_envelope_received(&MessageSource::Human);
    let action = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(action, Verdict::Continue),
        "expected Continue after Human envelope reset, got {action:?}"
    );
}

#[test]
fn loop_detector_scheduled_envelope_resets() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Read")];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
    }
    det.on_envelope_received(&MessageSource::Scheduled);
    let action = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(action, Verdict::Continue),
        "expected Continue after Scheduled envelope reset, got {action:?}"
    );
}

#[test]
fn loop_detector_system_envelope_does_not_reset() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Read")];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
    }
    // System-injected envelopes (continuation, hook rewake) must NOT reset —
    // they extend the current loop rather than mark a new task boundary.
    det.on_envelope_received(&MessageSource::System("goal_continuation".into()));
    let action = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(action, Verdict::AbortTurn { .. }),
        "expected AbortTurn (signatures preserved) after System envelope, got {action:?}"
    );
}

#[test]
fn loop_detector_different_tools_independent() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    // Read x2, Write x2 — neither reaches threshold
    det.on_before_tools(&mut ctx, &[tool("Read")]);
    det.on_before_tools(&mut ctx, &[tool("Write")]);
    det.on_before_tools(&mut ctx, &[tool("Read")]);
    let action = det.on_before_tools(&mut ctx, &[tool("Write")]);
    assert!(
        matches!(action, Verdict::Continue),
        "different tools should not trigger loop: {action:?}"
    );
}

#[test]
fn loop_detector_different_inputs_independent() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    // Same tool, different inputs — different signatures
    for i in 0..5 {
        let call = vec![(
            "id".into(),
            "Read".into(),
            json!({"file": format!("/tmp/{i}.rs")}),
        )];
        let action = det.on_before_tools(&mut ctx, &call);
        assert!(
            matches!(action, Verdict::Continue),
            "different inputs should not trigger loop at iteration {i}"
        );
    }
}

#[test]
fn loop_detector_multibyte_utf8_input_does_not_panic() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    // Large CJK input — we hash full JSON, so this only exercises UTF-8
    // safety of the serialized string. Must not panic.
    let cjk = "中".repeat(200); // 600 bytes
    let call = vec![("id".into(), "Write".into(), json!({"result": cjk}))];
    let action = det.on_before_tools(&mut ctx, &call);
    assert!(matches!(action, Verdict::Continue));
}

#[test]
fn loop_detector_on_compact_completed_resets_signatures() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Read")];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
    }
    det.on_compact_completed();
    let action = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(action, Verdict::Continue),
        "compact completion must reset signature counter; got {action:?}",
    );
}

#[test]
fn loop_detector_compact_reset_independent_from_envelope_reset() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Bash")];
    for _ in 0..2 {
        det.on_before_tools(&mut ctx, &calls);
    }
    det.on_compact_completed();
    // After compact reset, three more calls should not yet abort
    // (would only hit the WARN_THRESHOLD on the 3rd post-reset call).
    let a1 = det.on_before_tools(&mut ctx, &calls);
    let a2 = det.on_before_tools(&mut ctx, &calls);
    let a3 = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(a1, Verdict::Continue),
        "first post-compact call must Continue, got {a1:?}",
    );
    assert!(
        matches!(a2, Verdict::Continue),
        "second post-compact call must Continue, got {a2:?}",
    );
    assert!(
        matches!(a3, Verdict::InjectWarning(_)),
        "third post-compact call hits WARN_THRESHOLD as if starting fresh, got {a3:?}",
    );
}
