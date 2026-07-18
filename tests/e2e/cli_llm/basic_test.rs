use serde_json::json;

use crate::support::CliHarness;

/// The whole pipeline: prompt → real agent loop → Anthropic adapter → HTTP/SSE →
/// mock → streamed text back → Finished. Also asserts the wire the agent sent.
#[tokio::test]
async fn agent_completes_a_basic_text_turn_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "basic_text",
        "calls": [{
            "expect": {"userContains": "ping"},
            "chunks": [
                {"type": "text", "text": "pong"},
                {"type": "usage", "input": 5, "output": 2},
                {"type": "done"}
            ]
        }],
        "fallback": {"chunks": [{"type": "text", "text": "ok"}, {"type": "done"}]}
    }))
    .await;

    let out = h.run_turn("ping").await;
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
    assert!(out.text.contains("pong"), "unexpected text: {:?}", out.text);

    let journal = h.journal().await;
    assert_eq!(
        journal[0]["protocol"], "anthropic",
        "agent should reach the mock over the Anthropic wire; journal: {journal}"
    );
    assert!(
        journal[0]["lastUserText"]
            .as_str()
            .is_some_and(|t| t.contains("ping")),
        "journal should record the prompt; journal: {journal}"
    );
}
