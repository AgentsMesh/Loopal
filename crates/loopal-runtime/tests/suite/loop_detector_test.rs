use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_loop::cancel::TurnCancel;
use loopal_runtime::agent_loop::loop_detector::LoopDetector;
use loopal_runtime::agent_loop::turn_context::TurnContext;
use loopal_runtime::agent_loop::turn_observer::{ObserverAction, TurnObserver};
use serde_json::json;
use std::sync::Arc;

fn make_ctx() -> TurnContext {
    let cancel = TurnCancel::new(InterruptSignal::new(), Arc::new(tokio::sync::Notify::new()));
    TurnContext::new(0, cancel)
}

fn tool(name: &str) -> (String, String, serde_json::Value) {
    ("id".into(), name.into(), json!({"file": "/tmp/x.rs"}))
}

// --- TurnObserver trait defaults ---

#[test]
fn turn_observer_defaults_are_noop() {
    struct NoopObserver;
    impl TurnObserver for NoopObserver {}

    let mut obs = NoopObserver;
    let mut ctx = make_ctx();
    obs.on_turn_start(&mut ctx);
    let action = obs.on_before_tools(&mut ctx, &[tool("Read")]);
    assert!(matches!(action, ObserverAction::Continue));
    obs.on_after_tools(&mut ctx, &[tool("Read")], &[]);
    obs.on_turn_end(&ctx);
    obs.on_user_input();
}

// --- LoopDetector direct tests ---

#[test]
fn loop_detector_no_repeat_returns_continue() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let action = det.on_before_tools(&mut ctx, &[tool("Read")]);
    assert!(matches!(action, ObserverAction::Continue));
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
        matches!(action, ObserverAction::InjectWarning(_)),
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
    assert!(
        matches!(action, ObserverAction::AbortTurn(_)),
        "expected AbortTurn after 5 repeats, got {action:?}"
    );
}

#[test]
fn loop_detector_user_input_resets() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [tool("Read")];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
    }
    det.on_user_input();
    let action = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(action, ObserverAction::Continue),
        "expected Continue after reset, got {action:?}"
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
        matches!(action, ObserverAction::Continue),
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
            matches!(action, ObserverAction::Continue),
            "different inputs should not trigger loop at iteration {i}"
        );
    }
}
