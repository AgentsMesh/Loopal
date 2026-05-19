use loopal_protocol::{InterruptSignal, MessageSource, QualifiedAddress};
use loopal_runtime::agent_loop::cancel::TurnCancel;
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
        matches!(action, Verdict::Continue),
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
        matches!(action, Verdict::AbortTurn { .. }),
        "identical SendMessage repeated 5 times should still abort, got {action:?}"
    );
}

// --- Reset-on-envelope: end-to-end table sentinel ---

fn ready_to_abort_detector() -> (LoopDetector, TurnContext) {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [("id".into(), "Read".into(), json!({"file": "/tmp/x.rs"}))];
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
    }
    (det, ctx)
}

fn agent_addr() -> QualifiedAddress {
    "agent-x".into()
}

fn assert_reset_outcome(source: MessageSource, should_reset: bool, label: &str) {
    let (mut det, mut ctx) = ready_to_abort_detector();
    det.on_envelope_received(&source);
    let calls = [("id".into(), "Read".into(), json!({"file": "/tmp/x.rs"}))];
    let action = det.on_before_tools(&mut ctx, &calls);
    if should_reset {
        assert!(
            matches!(action, Verdict::Continue),
            "[{label}] expected Continue after reset, got {action:?}"
        );
    } else {
        assert!(
            matches!(action, Verdict::AbortTurn { .. }),
            "[{label}] expected AbortTurn (no reset), got {action:?}"
        );
    }
}

#[test]
fn loop_detector_envelope_reset_table() {
    // Each MessageSource variant is paired with its expected reset behavior.
    // When MessageSource (or the System-kind set) grows, the author must
    // extend this table and make an explicit decision.
    let cases: Vec<(MessageSource, bool, &str)> = vec![
        (MessageSource::Human, true, "Human"),
        (MessageSource::Scheduled, true, "Scheduled"),
        (MessageSource::Agent(agent_addr()), true, "Agent"),
        (
            MessageSource::Channel {
                channel: "main".into(),
                from: agent_addr(),
            },
            true,
            "Channel",
        ),
        (
            MessageSource::System("goal_continuation".into()),
            false,
            "System:goal_continuation",
        ),
        (
            MessageSource::System("governance_compensation".into()),
            false,
            "System:governance_compensation",
        ),
        (
            MessageSource::System("governance_feedback".into()),
            false,
            "System:governance_feedback",
        ),
        (
            MessageSource::System("stop_feedback".into()),
            false,
            "System:stop_feedback",
        ),
        (
            MessageSource::System("config_refresh".into()),
            false,
            "System:config_refresh",
        ),
        (
            MessageSource::System("compaction_summary".into()),
            false,
            "System:compaction_summary",
        ),
        (
            MessageSource::System("future_unknown".into()),
            false,
            "System:Other(unknown)",
        ),
    ];
    for (src, expected_reset, label) in cases {
        assert_reset_outcome(src, expected_reset, label);
    }
}
