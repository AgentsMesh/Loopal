use std::time::Duration;

use loopal_protocol::{
    AgentEventPayload, ControlCommand, Envelope, GateCloseReason, MessageSource,
};
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;

use super::e2e_event_waiters::{
    wait_for_gate_change, wait_for_interrupted_event, wait_for_running_event, wait_for_stream_event,
};
use super::goal_e2e_test::make_goal_session;

#[tokio::test]
async fn suspend_closes_gate_and_unsuspend_reopens() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("suspend-roundtrip").id);
    session.create("ongoing goal".into()).await.unwrap();

    let calls = vec![chunks::text_turn("first turn done")];
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::Suspend)
        .await
        .unwrap();
    let closed = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(closed.closed_reason, Some(GateCloseReason::UserSuspend));
    assert!(
        closed.wake_deadline.is_none(),
        "UserSuspend must not carry a wake deadline"
    );

    let baseline_calls = harness.recorded_messages.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let post_calls = harness.recorded_messages.lock().unwrap().len();
    assert_eq!(
        post_calls, baseline_calls,
        "Suspended: goal_continuation must not re-prompt the LLM"
    );

    harness
        .control_tx
        .send(ControlCommand::Unsuspend)
        .await
        .unwrap();
    let reopened = wait_for_gate_change(&mut harness.event_rx, true).await;
    assert!(reopened.open);
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}

#[tokio::test]
async fn human_input_in_suspended_auto_unsuspends() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("suspend-human").id);
    session.create("ongoing goal".into()).await.unwrap();

    let calls = vec![
        chunks::text_turn("first"),
        chunks::tool_turn("u1", "update_goal", json!({"status": "complete"})),
    ];
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::Suspend)
        .await
        .unwrap();
    let closed = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(closed.closed_reason, Some(GateCloseReason::UserSuspend));

    let envelope = Envelope::new(MessageSource::Human, "main", "wake up");
    harness.mailbox_tx.send(envelope).await.unwrap();

    wait_for_running_event(&mut harness.event_rx).await;
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}

#[tokio::test]
async fn infeasible_goal_stops_continuation_injection() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("infeasible-no-cont").id);
    session.create("blocked goal".into()).await.unwrap();

    let calls = vec![chunks::tool_turn(
        "ui1",
        "update_goal",
        json!({"status": "infeasible"}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    super::goal_e2e_test::wait_for_goal_reason(
        &log,
        loopal_protocol::GoalTransitionReason::ModelInfeasible,
    )
    .await;

    let after_terminal = harness.recorded_messages.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after_wait = harness.recorded_messages.lock().unwrap().len();
    assert_eq!(
        after_wait, after_terminal,
        "Infeasible goal must not participate in continuation: no further LLM calls expected"
    );
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}

#[tokio::test]
async fn esc_interrupt_leaves_session_in_waiting_for_input_not_suspended() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("esc-no-suspend").id);
    session.create("ongoing".into()).await.unwrap();

    let calls = vec![
        chunks::text_turn("first"),
        chunks::tool_turn("u1", "update_goal", json!({"status": "complete"})),
    ];
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .llm_chunk_delay(Duration::from_millis(80))
        .build_spawned()
        .await;

    // Synchronise on the first Stream event: this proves the LLM is
    // actively streaming the turn, so signalling interrupt now reliably
    // lands while the turn is in-flight (no sleep-based guesswork).
    wait_for_stream_event(&mut harness.event_rx).await;
    harness.interrupt.signal();
    wait_for_interrupted_event(&mut harness.event_rx).await;

    let mut gate_changed_after_interrupt = false;
    let drain_until = tokio::time::Instant::now() + Duration::from_millis(200);
    while tokio::time::Instant::now() < drain_until {
        if let Ok(Some(ev)) =
            tokio::time::timeout(Duration::from_millis(20), harness.event_rx.recv()).await
            && matches!(ev.payload, AgentEventPayload::ContinuationGateChanged(ref s) if !s.open)
        {
            gate_changed_after_interrupt = true;
        }
    }
    assert!(
        !gate_changed_after_interrupt,
        "ESC interrupt must NOT close ContinuationGate (only /suspend does)"
    );
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}
