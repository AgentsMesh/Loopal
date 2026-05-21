use std::time::Duration;

use loopal_protocol::{Envelope, GateCloseReason, MessageSource, ThreadGoalStatus};
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;

use super::e2e_event_waiters::{wait_for_gate_change, wait_for_recorded_text, wait_for_tool_error};
use super::goal_e2e_test::make_goal_session;

#[tokio::test]
async fn request_idle_closes_gate_with_deadline_and_keeps_goal_active() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("idle-closes-gate").id);
    session.create("long horizon goal".into()).await.unwrap();

    let calls = vec![chunks::tool_turn(
        "ri1",
        "request_idle",
        json!({
            "reason": "waiting for next cron fire",
            "max_idle_duration_secs": 1800,
            "expected_wake_signal": "cron",
        }),
    )];
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let summary = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(summary.closed_reason, Some(GateCloseReason::ModelRequested));
    assert!(summary.wake_deadline.is_some());

    let goal = session.snapshot().await.unwrap().expect("persisted");
    assert_eq!(
        goal.status,
        ThreadGoalStatus::Active,
        "request_idle must not mutate goal status"
    );
    drop(harness.control_tx);
}

#[tokio::test]
async fn request_idle_rejects_duration_below_minimum() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("idle-reject-min").id);
    session.create("x".into()).await.unwrap();

    let calls = vec![chunks::tool_turn(
        "ri-min",
        "request_idle",
        json!({"reason": "too short", "max_idle_duration_secs": 10}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let recorded = wait_for_tool_error(&harness.recorded_messages).await;
    assert!(
        recorded.contains("below the minimum"),
        "expected lower-bound rejection, got {recorded:?}"
    );
    drop(harness.control_tx);
}

#[tokio::test]
async fn closed_gate_blocks_goal_continuation_until_envelope_reopens() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("idle-gate-blocks").id);
    session.create("long horizon goal".into()).await.unwrap();

    let calls = vec![
        chunks::tool_turn(
            "ri1",
            "request_idle",
            json!({"reason": "waiting", "max_idle_duration_secs": 1800}),
        ),
        chunks::text_turn("processed external signal"),
    ];
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let closed = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(closed.closed_reason, Some(GateCloseReason::ModelRequested));

    let baseline_calls = harness.recorded_messages.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let post_idle_calls = harness.recorded_messages.lock().unwrap().len();
    assert_eq!(
        post_idle_calls, baseline_calls,
        "gate closed: no goal_continuation should re-prompt the LLM while idle"
    );

    let envelope = Envelope::new(MessageSource::Human, "main", "any update?");
    harness.mailbox_tx.send(envelope).await.unwrap();
    let reopened = wait_for_gate_change(&mut harness.event_rx, true).await;
    assert!(reopened.open);

    wait_for_recorded_text(
        &harness.recorded_messages,
        "any update?",
        Duration::from_secs(3),
    )
    .await;
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}

#[tokio::test]
async fn degeneration_then_envelope_reopens_gate_and_resumes() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("idle-degen-reopen").id);
    session.create("long-horizon".into()).await.unwrap();

    let same = "16/100. Not complete. No action.";
    let mut calls: Vec<_> = (0..8).map(|_| chunks::text_turn(same)).collect();
    calls.push(chunks::text_turn("woken by external signal"));
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let closed = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(closed.closed_reason, Some(GateCloseReason::Degeneration));

    let envelope = Envelope::new(MessageSource::Human, "main", "wake up please");
    harness.mailbox_tx.send(envelope).await.unwrap();
    let reopened = wait_for_gate_change(&mut harness.event_rx, true).await;
    assert!(reopened.open);
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}

#[tokio::test]
async fn degeneration_can_re_trigger_after_external_envelope_resets_silence() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("idle-degen-relapse").id);
    session.create("long-horizon".into()).await.unwrap();

    let dup_a = "16/100. Not complete. No action.";
    let dup_b = "still nothing.";
    let mut calls: Vec<_> = (0..8).map(|_| chunks::text_turn(dup_a)).collect();
    calls.extend((0..8).map(|_| chunks::text_turn(dup_b)));
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let first_close = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(
        first_close.closed_reason,
        Some(GateCloseReason::Degeneration)
    );

    let envelope = Envelope::new(MessageSource::Human, "main", "try again");
    harness.mailbox_tx.send(envelope).await.unwrap();
    let _ = wait_for_gate_change(&mut harness.event_rx, true).await;

    let second_close = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(
        second_close.closed_reason,
        Some(GateCloseReason::Degeneration),
        "detector must re-fire after external envelope clears the silenced flag"
    );
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}
