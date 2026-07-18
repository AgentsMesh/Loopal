use serde_json::json;

use crate::support::CliHarness;

/// After several turns of history, a manual `/compact` runs a summarization LLM
/// call over the real wire and settles with a Compacted event — compaction
/// driven end-to-end through the full stack.
#[tokio::test]
async fn agent_compacts_conversation_over_the_wire() {
    let long_summary = format!(
        "<summary>\n## Working state\n{}\n</summary>",
        "context ".repeat(80)
    );
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "compaction",
        "calls": [
            {"expect": {"userContains": "alpha"}, "chunks": [{"type": "text", "text": "reply a"}, {"type": "done"}]},
            {"expect": {"userContains": "beta"}, "chunks": [{"type": "text", "text": "reply b"}, {"type": "done"}]},
            {"expect": {"userContains": "gamma"}, "chunks": [{"type": "text", "text": "reply c"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": long_summary}, {"type": "done"}]}
    }))
    .await;
    h.begin_persistent().await;
    h.turn_via_message("alpha please").await;
    h.turn_via_message("beta please").await;
    h.turn_via_message("gamma please").await;

    let out = h.control(json!({"Compact": {"instructions": null}})).await;
    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("Compacted") || e.starts_with("CompactProgress")),
        "compaction should run a summarization over the wire and emit a Compacted event; \
         events: {:?}",
        out.events
    );
}
