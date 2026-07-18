use std::time::Duration;

use serde_json::json;

use crate::support::CliHarness;

/// Runtime control commands applied to a live session, each verified by its
/// effect on the wire: ModelSwitch changes the model of the next LLM request,
/// Rewind drops a turn from the history the next request carries, Clear
/// resets it entirely, and ModeSwitch / QueryMcpStatus surface their events.
#[tokio::test]
async fn runtime_controls_reshape_the_next_llm_request() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "controls",
        "calls": [
            {"expect": {"userContains": "first"},
             "chunks": [{"type": "text", "text": "one"}, {"type": "done"}]},
            {"expect": {"userContains": "second"},
             "chunks": [{"type": "text", "text": "two"}, {"type": "done"}]},
            {"expect": {"userContains": "third"},
             "chunks": [{"type": "text", "text": "three"}, {"type": "done"}]},
            {"expect": {"userContains": "fourth"},
             "chunks": [{"type": "text", "text": "four"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.begin_persistent().await;
    h.turn_via_message("first please").await;

    h.control_fire(json!({"ModelSwitch": "claude-e2e-switched"}))
        .await;
    assert!(
        h.await_event("ModelChanged", Duration::from_secs(3)).await,
        "ModelSwitch must emit ModelChanged"
    );
    h.turn_via_message("second please").await;

    h.control_fire(json!({"Rewind": {"turn_index": 1}})).await;
    assert!(
        h.await_event("Rewound", Duration::from_secs(3)).await,
        "Rewind must emit Rewound"
    );
    h.turn_via_message("third please").await;

    h.control_fire(json!("Clear")).await;
    assert!(
        h.await_event("Cleared", Duration::from_secs(3)).await,
        "Clear must emit Cleared"
    );
    h.turn_via_message("fourth please").await;

    h.control_fire(json!({"ModeSwitch": "Plan"})).await;
    assert!(
        h.await_event("ModeChanged", Duration::from_secs(3)).await,
        "ModeSwitch must emit ModeChanged"
    );
    h.control_fire(json!("QueryMcpStatus")).await;
    assert!(
        h.await_event("McpStatusReport", Duration::from_secs(3))
            .await,
        "QueryMcpStatus must answer with McpStatusReport"
    );

    let journal = h.journal().await;
    assert_eq!(
        journal[1]["model"], "claude-e2e-switched",
        "ModelSwitch must change the model of the next LLM request; journal: {journal}"
    );
    let mc = |i: usize| journal[i]["messageCount"].as_u64().unwrap_or(0);
    assert_eq!(
        mc(2),
        mc(1),
        "after rewinding away turn 2, turn 3's request must carry the same \
         history depth turn 2 had; journal: {journal}"
    );
    assert_eq!(
        mc(3),
        mc(0),
        "after Clear, the next request must start from an empty history \
         again; journal: {journal}"
    );
}
