//! E2E tests for AttemptCompletion tool.

use loopal_test_support::{HarnessBuilder, assertions, scenarios};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_attempt_completion_exits_loop() {
    let calls = scenarios::attempt_completion("All done!");
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let events = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&events, "AttemptCompletion");
    assertions::assert_has_tool_result(&events, "AttemptCompletion", false);
    assertions::assert_has_finished(&events);
}

#[tokio::test]
async fn test_completion_result_accessible() {
    // Use IntegrationHarness (non-spawned) to inspect runner output.
    let calls = scenarios::attempt_completion("Task completed successfully.");
    let mut h = HarnessBuilder::new().calls(calls).build().await;

    // Drain events in background so the runner doesn't block.
    let mut rx = h.event_rx;
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let output = h.runner.run().await.unwrap();
    assert!(
        output.result.contains("Task completed successfully."),
        "expected result to contain completion text, got: {:?}",
        output.result
    );
}
