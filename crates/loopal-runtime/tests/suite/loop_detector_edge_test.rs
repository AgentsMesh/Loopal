use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_loop::cancel::TurnCancel;
use loopal_runtime::agent_loop::loop_detector::LoopDetector;
use loopal_runtime::agent_loop::turn_context::TurnContext;
use loopal_runtime::agent_loop::turn_observer::{ObserverAction, TurnObserver};
use serde_json::json;
use std::sync::Arc;

fn make_ctx() -> TurnContext {
    let cancel = TurnCancel::new(
        InterruptSignal::new(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    TurnContext::new(0, cancel)
}

// --- Regression: fan-out with long shared prefix must not collide ---

#[test]
fn loop_detector_fanout_different_targets_does_not_trigger() {
    // Regression for prefix-hash collision. When the signature was built
    // from the first 200 bytes of the serialized JSON, and `serde_json`
    // ordered keys alphabetically (BTreeMap), a SendMessage call with
    // {"message": <long>, "summary": …, "to": <target>} would hash away
    // the `to` field entirely — so 5 messages to 5 distinct recipients
    // looked identical and tripped the abort threshold.
    //
    // With full-JSON hashing, each distinct `to` yields a distinct signature.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let long_msg = "你好。我是 hub-83e6571f 的 agent。用户给我布置了一个任务：".repeat(6);
    let targets = [
        "hub-6d7d3682",
        "hub-0d7124fc",
        "hub-9b54624e",
        "hub-4809c5a6",
        "hub-f117ce0b",
    ];
    let calls: Vec<(String, String, serde_json::Value)> = targets
        .iter()
        .map(|t| {
            (
                format!("id-{t}"),
                "SendMessage".into(),
                json!({"to": *t, "message": long_msg, "summary": "intro ping"}),
            )
        })
        .collect();

    let action = det.on_before_tools(&mut ctx, &calls);
    assert!(
        matches!(action, ObserverAction::Continue),
        "fan-out to 5 distinct targets must not trigger loop detector, got {action:?}"
    );
}

#[test]
fn loop_detector_fanout_with_identical_payload_still_triggers() {
    // Sanity check: the fix must not mask genuine loops. Repeating the
    // *exact same* call (identical `to` + `message`) 5 times should still
    // abort — this is the behavior the detector was designed to protect.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let call = vec![(
        "id".into(),
        "SendMessage".into(),
        json!({"to": "hub-a", "message": "hello", "summary": "s"}),
    )];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &call);
    }
    let action = det.on_before_tools(&mut ctx, &call);
    assert!(
        matches!(action, ObserverAction::AbortTurn(_)),
        "identical SendMessage repeated 5 times should still abort, got {action:?}"
    );
}
