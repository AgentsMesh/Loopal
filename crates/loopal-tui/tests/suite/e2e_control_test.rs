//! E2E tests for control commands (Clear, Compact, ThinkingSwitch,
//! AutoContinuation, Rewind).

use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, assertions, events, scenarios};
use loopal_tui::app::App;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

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
async fn test_clear_command() {
    let calls = scenarios::n_turn(&["First response.", "After clear."]);
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

    // Complete first turn
    let ev1 = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev1);

    // Snapshot pre-clear conversation has the user prompt + assistant reply
    let pre_messages = harness.app.snapshot_active_conversation().messages.len();
    assert!(
        pre_messages > 0,
        "expected non-empty conversation before clear"
    );

    // Send Clear — view-state must drop every persisted row before the
    // next turn lands. Wait for the Cleared event to round-trip back.
    harness
        .inner
        .control_tx
        .send(ControlCommand::Clear)
        .await
        .unwrap();
    let clear_evts = harness
        .collect_until(|e| matches!(e, AgentEventPayload::Cleared { .. }))
        .await;
    assert!(
        clear_evts
            .iter()
            .any(|e| matches!(e, AgentEventPayload::Cleared { .. })),
        "expected Cleared event in stream"
    );

    let conv = harness.app.snapshot_active_conversation();
    assert!(
        conv.messages.is_empty(),
        "conversation.messages must be empty after Clear, got {} rows",
        conv.messages.len()
    );
    assert_eq!(conv.turn_count, 0, "turn_count must reset to 0");
    assert_eq!(conv.input_tokens, 0, "input_tokens must reset to 0");
    assert_eq!(conv.output_tokens, 0, "output_tokens must reset to 0");
    let obs = harness.app.observable_for("main");
    assert_eq!(
        harness.app.tool_count_for("main"),
        0,
        "tool_count must reset"
    );
    assert!(
        harness.app.last_tool_for("main").is_none(),
        "last_tool must be cleared"
    );
    let _ = obs;

    // Next turn should still run cleanly.
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

    let _ = harness.collect_until_idle().await;

    harness
        .inner
        .control_tx
        .send(ControlCommand::Compact { instructions: None })
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
