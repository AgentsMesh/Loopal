use serde_json::json;

use crate::support::CliHarness;

/// Thinking/reasoning chunks stream back as `ThinkingStream` events over the wire.
#[tokio::test]
async fn agent_surfaces_thinking_stream_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "thinking",
        "calls": [{
            "expect": {"userContains": "think about it"},
            "chunks": [
                {"type": "thinking", "text": "let me reason "},
                {"type": "text", "text": "answer"},
                {"type": "done"}
            ]
        }]
    }))
    .await;

    let out = h.run_turn("think about it").await;
    assert!(out.finished, "events: {:?}", out.events);
    assert!(out.text.contains("answer"), "text: {:?}", out.text);
    assert!(
        out.thinking.contains("let me reason"),
        "thinking should stream back; thinking: {:?}\nevents: {:?}",
        out.thinking,
        out.events
    );
}

/// Two tool calls in one response are both executed before the turn continues —
/// the parallel tool path, over the wire.
#[tokio::test]
async fn agent_runs_parallel_tools_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "parallel_tools",
        "calls": [
            {"expect": {"userContains": "two tools"}, "chunks": [
                {"type": "tool_use", "id": "a", "name": "Bash", "input": {"command": "echo aaa"}},
                {"type": "tool_use", "id": "b", "name": "Bash", "input": {"command": "echo bbb"}},
                {"type": "done"}
            ]},
            {"expect": {}, "chunks": [{"type": "text", "text": "both tools done"}, {"type": "done"}]}
        ]
    }))
    .await;

    let out = h.run_turn("run two tools").await;
    assert!(out.finished, "events: {:?}", out.events);
    assert!(out.text.contains("both tools done"), "text: {:?}", out.text);
    assert!(
        out.tool_result_count() >= 2,
        "both tool calls should execute; events: {:?}",
        out.events
    );
}
