use serde_json::json;

use crate::support::HubHarness;

/// A real sub-agent turn through the full topology: the root agent's Agent
/// tool asks the Hub to spawn a child agent PROCESS, the child runs its own
/// LLM turn against the same mock, and its answer flows back into the root's
/// tool result and continuation.
#[tokio::test]
async fn agent_tool_spawns_a_real_child_process_turn() {
    let mut h = HubHarness::start(json!({
        "version": 2,
        "name": "subagent",
        "calls": [
            {"expect": {"userContains": "delegate the subtask"},
             "chunks": [
                {"type": "tool_use", "id": "a1", "name": "Agent",
                 "input": {"prompt": "solve magic-subtask-424242 and report",
                           "name": "subworker"}},
                {"type": "done"}
             ]},
            {"expect": {"userContains": "magic-subtask-424242"},
             "chunks": [{"type": "text", "text": "child-answer-777"}, {"type": "done"}]},
            {"expect": {"toolResultId": "a1"},
             "chunks": [{"type": "text", "text": "delegation complete"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.turn("please delegate the subtask").await;
    assert!(
        out.error.is_none() && out.finished,
        "turn failed: {:?}\nevents: {:?}",
        out.error,
        out.events
    );
    assert!(
        out.text.contains("delegation complete"),
        "the root continuation only fires after the child's result returned; \
         text: {:?}\nevents: {:?}",
        out.text,
        out.events
    );
    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("child-answer-777")),
        "the child's LLM answer must come back through the Agent tool result; \
         events: {:?}",
        out.events
    );
    assert!(
        out.events.iter().any(|e| e.starts_with("SubAgentSpawned")),
        "spawning must surface a SubAgentSpawned event; events: {:?}",
        out.events
    );

    let journal = h.journal().await;
    assert!(
        journal.as_array().is_some_and(|calls| calls.len() >= 3),
        "root + child + continuation means at least three LLM calls; \
         journal: {journal}"
    );
}
