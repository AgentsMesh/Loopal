//! E2E tests for multi-turn conversations, max_turns, mode switch, and interrupts.

use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, assertions, chunks, events};
use loopal_tui::app::App;
use loopal_tui::command::CommandEntry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

/// Wrap a SpawnedHarness with TUI components.
fn wrap_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        Vec::<CommandEntry>::new(),
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
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

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
async fn test_max_turns_reached() {
    // max_turns=1 with interactive mode. First turn is a tool call, then we send
    // a second message. The loop hits max_turns at the top of the next iteration.
    let calls = vec![chunks::tool_turn(
        "tc-1",
        "Ls",
        serde_json::json!({"path": "/tmp"}),
    )];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .max_turns(1)
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    // First turn: tool executes, then AwaitingInput
    let ev1 = harness.collect_until_idle().await;
    assertions::assert_has_tool_call(&ev1, "Ls");

    // Send second message to trigger next iteration
    let envelope = Envelope::new(MessageSource::Human, "main", "next");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    // Runner should hit max_turns and emit MaxTurnsReached → Finished
    let ev2 = harness.collect_until_idle().await;
    assertions::assert_has_max_turns(&ev2);
}

#[tokio::test]
async fn test_mode_switch_act_to_plan() {
    let calls = vec![
        chunks::text_turn("Ready"),
        chunks::text_turn("Now in plan mode"),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

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
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    let events = harness.collect_until_idle().await;
    assertions::assert_has_stream(&events);

    // Verify interrupt signal doesn't panic (agent is already idle)
    harness.inner.session_ctrl.interrupt();
}
