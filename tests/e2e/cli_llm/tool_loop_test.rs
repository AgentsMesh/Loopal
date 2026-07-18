use serde_json::json;

use crate::support::CliHarness;

/// Full tool round-trip over the wire: the model asks for a tool, the real agent
/// executes it, sends the result back, and the model's follow-up call (matched on
/// the returned tool-result id) completes the turn. The final text only appears
/// if every step of the loop worked end-to-end.
#[tokio::test]
async fn agent_runs_a_tool_and_continues_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "tool_loop",
        "calls": [
            {"expect": {"userContains": "run the tool"},
             "chunks": [
                {"type": "tool_use", "id": "t1", "name": "Bash",
                 "input": {"command": "echo tool-ran-ok"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "t1"},
             "chunks": [{"type": "text", "text": "the tool finished"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.run_turn("run the tool please").await;
    assert!(
        out.error.is_none(),
        "turn errored: {:?}\nevents: {:?}",
        out.error,
        out.events
    );
    assert!(
        out.finished,
        "turn did not finish; events: {:?}",
        out.events
    );
    assert!(
        out.text.contains("the tool finished"),
        "the tool-result follow-up call only returns this text if the whole loop \
         (tool_use → execute → tool_result → matched continuation) worked; text: {:?}",
        out.text
    );

    let journal = h.journal().await;
    assert!(
        journal.as_array().is_some_and(|calls| calls.len() >= 2),
        "expected at least the initial + tool-result calls; journal: {journal}"
    );
}
