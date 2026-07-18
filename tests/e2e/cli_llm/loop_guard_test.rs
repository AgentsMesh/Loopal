use serde_json::json;

use crate::support::CliHarness;

fn identical_call(prev_id: &str, id: &str) -> serde_json::Value {
    json!({"expect": {"toolResultId": prev_id},
    "chunks": [
       {"type": "tool_use", "id": id, "name": "Bash",
        "input": {"command": "echo loop-body"}},
       {"type": "done"}
    ]})
}

/// Degeneration guard through the full stack: after three completed identical
/// call→result rounds, the fourth identical tool call trips the loop
/// detector's warn threshold, injecting a corrective warning into the
/// conversation while the turn still completes.
#[tokio::test]
async fn repeated_identical_tool_calls_inject_a_loop_warning() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "loop_guard",
        "calls": [
            {"expect": {"userContains": "start looping"},
             "chunks": [
                {"type": "tool_use", "id": "l1", "name": "Bash",
                 "input": {"command": "echo loop-body"}},
                {"type": "done"}
             ]},
            identical_call("l1", "l2"),
            identical_call("l2", "l3"),
            identical_call("l3", "l4"),
            {"expect": {"toolResultId": "l4"},
             "chunks": [{"type": "text", "text": "stopped looping"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.run_turn("start looping please").await;
    assert!(
        out.finished && out.text.contains("stopped looping"),
        "turn failed: {out:?}"
    );
    assert_eq!(out.tool_result_count(), 4, "all four calls ran: {out:?}");
    let injected = out
        .events
        .iter()
        .find(|e| e.starts_with("TurnCompleted"))
        .map(|e| e.contains("warnings_injected: 0"))
        .unwrap_or(true);
    assert!(
        !injected,
        "the fourth identical call must arrive with a loop warning injected; \
         events: {:?}",
        out.events
    );

    let journal = h.journal().await.to_string();
    assert!(
        journal.contains("stuck in a loop"),
        "the injected warning must reach the model over the wire; \
         journal: {journal}"
    );
}
