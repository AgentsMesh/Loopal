use serde_json::json;

use crate::support::CliHarness;

/// Two turns in one persistent session, the second delivered as a follow-up user
/// message, must carry the first turn's history (message count grows) — session
/// continuity over the wire.
#[tokio::test]
async fn session_carries_context_across_turns_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "multi_turn",
        "calls": [
            {"expect": {"userContains": "first"}, "chunks": [{"type": "text", "text": "one"}, {"type": "done"}]},
            {"expect": {"userContains": "second"}, "chunks": [{"type": "text", "text": "two"}, {"type": "done"}]}
        ]
    }))
    .await;
    h.begin_persistent().await;

    let out1 = h.turn_via_message("first message").await;
    assert!(
        out1.finished && out1.text.contains("one"),
        "turn 1: {out1:?}"
    );
    let out2 = h.turn_via_message("second message").await;
    assert!(
        out2.finished && out2.text.contains("two"),
        "turn 2: {out2:?}"
    );

    let journal = h.journal().await;
    let first = journal[0]["messageCount"].as_u64().unwrap_or(0);
    let second = journal[1]["messageCount"].as_u64().unwrap_or(0);
    assert!(
        second > first,
        "the second turn must include the first turn's history; journal: {journal}"
    );
}
