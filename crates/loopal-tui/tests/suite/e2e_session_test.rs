//! E2E session tests: persistence roundtrip, model switch, empty input.

use loopal_protocol::{ControlCommand, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, assertions, chunks};
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
async fn test_session_persistence_roundtrip() {
    // Use IntegrationHarness (not spawned) so we can call runner.run() directly.
    let harness = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("persisted response")])
        .build()
        .await;

    // The session dir is <fixture>/sessions/sessions/integration-test/
    // (session_manager base_dir = <fixture>/sessions, storage adds sessions/<id>/)
    let session_dir = harness
        .fixture
        .path()
        .join("sessions/sessions/integration-test");

    let mut runner = harness.runner;
    let _ = runner.run().await;

    // The runner saves messages via session_manager.save_message().
    // Check that the session directory was created with message files.
    assert!(
        session_dir.exists(),
        "session directory should exist at {}",
        session_dir.display()
    );
    let entries: Vec<_> = std::fs::read_dir(&session_dir)
        .expect("read session dir")
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !entries.is_empty(),
        "session directory should contain message files"
    );
}

#[tokio::test]
async fn test_model_switch_mid_session() {
    let calls = vec![
        chunks::text_turn("First turn"),
        chunks::text_turn("Second turn after model switch"),
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

    // Send model switch, then yield to let the runner process it
    harness
        .inner
        .control_tx
        .send(ControlCommand::ModelSwitch("gpt-4o".into()))
        .await
        .unwrap();
    // Allow runner to process control before we send the next message
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send next message
    let envelope = Envelope::new(MessageSource::Human, "main", "continue");
    harness.inner.mailbox_tx.send(envelope).await.unwrap();

    // Second turn should succeed (no crash)
    let ev2 = harness.collect_until_idle().await;
    // After model switch, the runner continues to use the mock provider.
    // If model switch caused any issue, we'd see an error or panic.
    // Just verify we got either stream text or finished without crash.
    let has_events = !ev2.is_empty();
    assert!(
        has_events,
        "second turn should produce events after model switch"
    );
}

#[tokio::test]
async fn test_empty_user_input() {
    // Empty message should not crash the agent loop
    let calls = vec![chunks::text_turn("Response to empty")];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![loopal_message::Message::user("")])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    let evts = harness.collect_until_idle().await;
    // Should reach idle without panic
    assertions::assert_has_finished(&evts);
}
