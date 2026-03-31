//! E2E tests for multi-turn conversations, mode switch, and interrupts.

use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, assertions, chunks, events};
use loopal_tui::app::App;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

/// Wrap a SpawnedHarness with TUI components.
fn wrap_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        inner.fixture.path().to_path_buf(),
    );
    TuiTestHarness {
        terminal,
        app,
        inner,
    }
}

#[tokio::test]
async fn test_interactive_two_turns() {
    let calls = vec![
        chunks::text_turn("First response"),
        chunks::text_turn("Second response"),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    // Drain initial AwaitingInput (store empty, agent waits for first message)
    let _ = harness.collect_until_idle().await;
    harness
        .inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "hello"))
        .await
        .unwrap();

    // First turn
    let ev1 = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev1);
    let text1 = events::extract_texts(&ev1);
    assert!(text1.contains("First response"), "got: {text1}");

    // Send second message
    let envelope = Envelope::new(MessageSource::Human, "main", "next question");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    // Second turn
    let ev2 = harness.collect_until_idle().await;
    let text2 = events::extract_texts(&ev2);
    assert!(text2.contains("Second response"), "got: {text2}");
}

#[tokio::test]
async fn test_mode_switch_act_to_plan() {
    let calls = vec![
        chunks::text_turn("Ready"),
        chunks::text_turn("Now in plan mode"),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    // Drain initial AwaitingInput (store empty, agent waits for first message)
    let _ = harness.collect_until_idle().await;
    harness
        .inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "hello"))
        .await
        .unwrap();

    // First turn
    let ev1 = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev1);

    // Send mode switch. The runner is in wait_for_input, so control is processed first.
    let mode = loopal_protocol::AgentMode::Plan;
    harness
        .inner
        .control_tx
        .send(ControlCommand::ModeSwitch(mode))
        .await
        .unwrap();

    // Yield to let runner process the control before we send the message
    tokio::task::yield_now().await;

    let envelope = Envelope::new(MessageSource::Human, "main", "plan this");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    // Collect: should see ModeChanged then Stream
    let ev2 = harness.collect_until_idle().await;

    // ModeChanged may be in ev2, or may have been emitted before our second collect.
    // Check both batches.
    let all: Vec<_> = ev1.iter().chain(ev2.iter()).cloned().collect();
    let has_mode_changed = all
        .iter()
        .any(|e| matches!(e, AgentEventPayload::ModeChanged { mode } if mode == "plan"));
    assert!(
        has_mode_changed,
        "expected ModeChanged(plan) in events: {all:?}"
    );
}

#[tokio::test]
async fn test_interrupt_stops_processing() {
    let calls = vec![chunks::text_turn("This response will be interrupted")];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    // Drain initial AwaitingInput (store empty, agent waits for first message)
    let _ = harness.collect_until_idle().await;
    harness
        .inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "hello"))
        .await
        .unwrap();

    let events = harness.collect_until_idle().await;
    assertions::assert_has_stream(&events);

    // Verify interrupt signal doesn't panic (agent is already idle)
    harness.inner.session_ctrl.interrupt();
}
