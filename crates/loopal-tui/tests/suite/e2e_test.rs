//! TUI integration tests — full provider → agent_loop → session → render chain.

use loopal_test_support::{TestFixture, assertions, chunks};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_text_response_rendered() {
    let mut harness = build_tui_harness(vec![chunks::text_turn("Hello from agent!")], 80, 24).await;
    let events = harness.collect_until_idle().await;

    assertions::assert_has_stream(&events);
    assertions::assert_buffer_contains(&harness.render_text(), "Hello from agent!");
}

#[tokio::test]
async fn test_tool_call_then_text() {
    // Separate fixture for the file — auto-cleaned on drop.
    let file_fixture = TestFixture::new();
    let file_path = file_fixture.create_file("test.txt", "test content");

    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "Read",
            serde_json::json!({"file_path": file_path.to_str().unwrap()}),
        ),
        chunks::text_turn("File read done."),
    ];

    let mut harness = build_tui_harness(calls, 100, 30).await;
    let events = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&events, "Read");
    assertions::assert_buffer_contains(&harness.render_text(), "File read done.");
}

#[tokio::test]
async fn test_finished_event() {
    let mut harness = build_tui_harness(vec![chunks::text_turn("Done.")], 80, 24).await;
    let events = harness.collect_until_idle().await;
    assertions::assert_has_finished(&events);
}
