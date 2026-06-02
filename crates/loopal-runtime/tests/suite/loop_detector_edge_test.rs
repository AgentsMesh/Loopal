use loopal_protocol::{InterruptSignal, MessageSource, QualifiedAddress};
use loopal_provider_api::ContentBlock;
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

fn res(id: &str, content: &str) -> Vec<ContentBlock> {
    vec![ContentBlock::ToolResult {
        tool_use_id: id.into(),
        content: content.into(),
        images: vec![],
        is_error: false,
        metadata: None,
    }]
}

#[test]
fn fanout_different_targets_does_not_trigger() {
    // Each distinct `to` yields a distinct input signature, so fanning out to
    // many recipients never accrues a shared streak (regression for an old
    // prefix-hash collision).
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let long_msg = "你好。我是 hub 的 agent。用户给我布置了任务：".repeat(6);
    let targets = ["hub-a", "hub-b", "hub-c", "hub-d", "hub-e"];
    for t in targets {
        let calls = vec![(
            format!("id-{t}"),
            "SendMessage".into(),
            json!({"to": t, "message": long_msg, "summary": "intro"}),
        )];
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &res(&format!("id-{t}"), "ok"));
    }
    let calls = vec![(
        "id-hub-a".into(),
        "SendMessage".into(),
        json!({"to": "hub-a", "message": long_msg, "summary": "intro"}),
    )];
    assert!(matches!(
        det.on_before_tools(&mut ctx, &calls),
        Verdict::Continue
    ));
}

#[test]
fn identical_payload_and_output_still_triggers() {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let call = vec![(
        "id".into(),
        "SendMessage".into(),
        json!({"to": "hub-a", "message": "hello", "summary": "s"}),
    )];
    for _ in 0..5 {
        det.on_before_tools(&mut ctx, &call);
        det.on_after_tools(&mut ctx, &call, &res("id", "same"));
    }
    assert!(matches!(
        det.on_before_tools(&mut ctx, &call),
        Verdict::AbortTurn { .. }
    ));
}

fn ready_to_abort_detector() -> (LoopDetector, TurnContext) {
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [("id".into(), "Read".into(), json!({"file": "/tmp/x.rs"}))];
    for _ in 0..5 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &res("id", "same"));
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
fn envelope_reset_table() {
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
            MessageSource::System("governance_feedback".into()),
            false,
            "System:governance_feedback",
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

#[test]
fn repeated_identical_error_aborts() {
    // A tool that keeps returning the SAME error (is_error + same content) is a
    // real loop — must still abort after the threshold.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = [("id".into(), "Bash".into(), json!({"cmd": "x"}))];
    let err = vec![ContentBlock::ToolResult {
        tool_use_id: "id".into(),
        content: "boom".into(),
        images: vec![],
        is_error: true,
        metadata: None,
    }];
    for _ in 0..5 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &err);
    }
    assert!(matches!(
        det.on_before_tools(&mut ctx, &calls),
        Verdict::AbortTurn { .. }
    ));
}

#[test]
fn parallel_identical_calls_count_once_per_batch() {
    // Two identical (name,input) calls in one batch must bump the streak by 1,
    // not 2 — a batch is a single decision point.
    let mut det = LoopDetector::new();
    let mut ctx = make_ctx();
    let calls = vec![
        ("id1".into(), "Read".into(), json!({"file": "/tmp/x.rs"})),
        ("id2".into(), "Read".into(), json!({"file": "/tmp/x.rs"})),
    ];
    let results = [res("id1", "same"), res("id2", "same")].concat();
    // With per-batch dedup, 4 batches → count 4 → InjectWarning (≥WARN 3, not
    // yet ABORT 5). Without dedup, 4 × 2 = 8 would abort; if accrual broke
    // entirely, count would stay 0 → Continue. Asserting Warning pins both.
    for _ in 0..4 {
        det.on_before_tools(&mut ctx, &calls);
        det.on_after_tools(&mut ctx, &calls, &results);
    }
    assert!(matches!(
        det.on_before_tools(&mut ctx, &calls),
        Verdict::InjectWarning(_)
    ));
}
