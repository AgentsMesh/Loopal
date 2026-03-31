//! E2E edge-case tests: rate limit, large output truncation, rapid inputs, tool chain.

use loopal_protocol::{AgentEventPayload, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, TestFixture, assertions, chunks, events};
use loopal_tui::app::App;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::{TuiTestHarness, build_tui_harness};

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
async fn test_rate_limit_error() {
    let calls = vec![vec![chunks::rate_limited(5000)]];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_error(&evts);
}

#[tokio::test]
async fn test_large_output_truncation() {
    let fixture = TestFixture::new();
    // Create a >100KB file to trigger truncation in the tool pipeline
    let large_content = "x".repeat(120_000);
    let file_path = fixture.create_file("large.txt", &large_content);
    let path_str = file_path.to_str().unwrap().to_string();

    let calls = vec![
        chunks::tool_turn("tc-r", "Read", serde_json::json!({"file_path": path_str})),
        chunks::text_turn("Read done."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    // Read should succeed (not an error, just truncated)
    assertions::assert_has_tool_result(&evts, "Read", false);

    // Check that output was truncated (contains truncation marker or is < original)
    let read_output: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "Read" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    // The Read tool or pipeline should truncate; result should be smaller than original
    assert!(
        read_output.iter().any(|r| r.len() < large_content.len()),
        "output should be truncated (shorter than 120KB)"
    );
}

#[tokio::test]
async fn test_rapid_consecutive_inputs() {
    // Interactive mode with 3 turns, send all 3 messages rapidly
    let calls = vec![
        chunks::text_turn("Response 1"),
        chunks::text_turn("Response 2"),
        chunks::text_turn("Response 3"),
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

    // First turn (initial message from builder)
    let ev1 = harness.collect_until_idle().await;
    let text1 = events::extract_texts(&ev1);
    assert!(text1.contains("Response 1"), "turn 1: got {text1}");

    // Send 2 messages rapidly before processing
    let e1 = Envelope::new(MessageSource::Human, "main", "msg2");
    let e2 = Envelope::new(MessageSource::Human, "main", "msg3");
    harness.inner.mailbox_tx.send(e1).await.unwrap();
    harness.inner.mailbox_tx.send(e2).await.unwrap();

    // Collect turn 2
    let ev2 = harness.collect_until_idle().await;
    let text2 = events::extract_texts(&ev2);
    assert!(text2.contains("Response 2"), "turn 2: got {text2}");

    // Collect turn 3
    let ev3 = harness.collect_until_idle().await;
    let text3 = events::extract_texts(&ev3);
    assert!(text3.contains("Response 3"), "turn 3: got {text3}");
}

#[tokio::test]
async fn test_tool_chain_two_turns() {
    // Turn 1: tool call (Read), Turn 2: auto-continue with text (non-interactive)
    let fixture = TestFixture::new();
    fixture.create_file("chain.txt", "chain content");
    let path_str = fixture
        .path()
        .join("chain.txt")
        .to_str()
        .unwrap()
        .to_string();

    let calls = vec![
        chunks::tool_turn("tc-r", "Read", serde_json::json!({"file_path": path_str})),
        chunks::text_turn("Chain complete."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "Read");
    assertions::assert_has_tool_result(&evts, "Read", false);
    assertions::assert_has_stream(&evts);
    assertions::assert_has_finished(&evts);
}
