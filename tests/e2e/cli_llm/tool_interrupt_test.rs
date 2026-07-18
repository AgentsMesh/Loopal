use std::time::Duration;

use serde_json::json;

use crate::support::CliHarness;

/// Interrupting DURING tool execution (not during the LLM stream): a Bash
/// `sleep 8` is underway when `agent/interrupt` lands; the turn must settle
/// cancelled well before the sleep would have finished.
#[tokio::test]
async fn interrupt_during_tool_execution_cancels_promptly() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "tool_interrupt",
        "calls": [
            {"expect": {"userContains": "run the sleeper"},
             "chunks": [
                {"type": "tool_use", "id": "ti1", "name": "Bash",
                 "input": {"command": "sleep 8"}},
                {"type": "done"}
             ]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    h.begin_turn("please run the sleeper").await;
    assert!(
        h.await_event("ToolCall", Duration::from_secs(10)).await,
        "the Bash call must start"
    );
    let started = tokio::time::Instant::now();
    h.interrupt().await;
    let out = h.await_settled(Duration::from_secs(10)).await;

    assert!(
        out.cancelled,
        "the in-flight tool turn must settle as cancelled; out: {out:?}"
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(6),
        "cancellation must not wait out the 8s sleep; took {elapsed:?}"
    );
}
