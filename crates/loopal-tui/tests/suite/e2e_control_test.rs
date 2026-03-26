//! E2E tests for control commands (Clear, Compact, ThinkingSwitch,
//! AutoContinuation, Rewind).

use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, assertions, events, scenarios};
use loopal_tui::app::App;
use loopal_tui::command::CommandEntry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

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
async fn test_clear_command() {
    let calls = scenarios::n_turn(&["First response.", "After clear."]);
    let inner = HarnessBuilder::new()
        .calls(calls)
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    // Collect first turn
    let ev1 = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev1);

    // Send Clear then next message
    harness
        .inner
        .control_tx
        .send(ControlCommand::Clear)
        .await
        .unwrap();
    tokio::task::yield_now().await;

    let envelope = Envelope::new(MessageSource::Human, "main", "continue");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    let ev2 = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev2);
}

#[tokio::test]
async fn test_compact_command() {
    let calls = scenarios::two_turn("First.", "After compact.");
    let inner = HarnessBuilder::new()
        .calls(calls)
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    let _ = harness.collect_until_idle().await;

    harness
        .inner
        .control_tx
        .send(ControlCommand::Compact)
        .await
        .unwrap();
    tokio::task::yield_now().await;

    let envelope = Envelope::new(MessageSource::Human, "main", "go");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    let ev = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev);
}

#[tokio::test]
async fn test_thinking_switch() {
    let calls = scenarios::two_turn("Before switch.", "After switch.");
    let inner = HarnessBuilder::new()
        .calls(calls)
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    let _ = harness.collect_until_idle().await;

    let json = serde_json::json!({"type": "disabled"}).to_string();
    harness
        .inner
        .control_tx
        .send(ControlCommand::ThinkingSwitch(json))
        .await
        .unwrap();
    tokio::task::yield_now().await;

    let envelope = Envelope::new(MessageSource::Human, "main", "go");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    let ev = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev);
}

#[tokio::test]
async fn test_auto_continuation() {
    let calls = scenarios::auto_continuation("partial ", "complete.");
    let inner = HarnessBuilder::new().calls(calls).build_spawned().await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    let has_continuation = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::AutoContinuation { .. }));
    assert!(
        has_continuation,
        "expected AutoContinuation event: {evts:?}"
    );
    let text = events::extract_texts(&evts);
    assert!(
        text.contains("partial") && text.contains("complete"),
        "got: {text}"
    );
}

#[tokio::test]
async fn test_rewind_command() {
    let calls = scenarios::n_turn(&["Turn 1.", "Turn 2.", "Turn 3."]);
    let inner = HarnessBuilder::new()
        .calls(calls)
        .interactive(true)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    // Complete first turn
    let _ = harness.collect_until_idle().await;

    // Send first follow-up message
    let envelope = Envelope::new(MessageSource::Human, "main", "msg1");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();
    let _ = harness.collect_until_idle().await;

    // Rewind to turn 0
    harness
        .inner
        .control_tx
        .send(ControlCommand::Rewind { turn_index: 0 })
        .await
        .unwrap();
    tokio::task::yield_now().await;

    // Send another message after rewind
    let envelope = Envelope::new(MessageSource::Human, "main", "after rewind");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    let ev = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev);
}
