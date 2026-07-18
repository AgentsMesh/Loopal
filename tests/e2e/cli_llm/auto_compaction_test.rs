use serde_json::json;

use crate::support::CliHarness;

/// Auto-compaction driven by provider-reported usage: turn one's usage claims
/// 980k input tokens (98% of the 1M window), so the next turn boundary must
/// trigger an automatic summarization pass over the wire before proceeding.
#[tokio::test]
async fn provider_usage_past_threshold_triggers_auto_compaction() {
    let long_summary = format!(
        "<summary>\n## Working state\n{}\n</summary>",
        "context ".repeat(80)
    );
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "auto_compact",
        "calls": [
            {"expect": {"userContains": "heavy turn"},
             "chunks": [
                {"type": "usage", "input": 980000},
                {"type": "text", "text": "reply a"},
                {"type": "done"}
             ]},
            {"expect": {}, "chunks": [{"type": "text", "text": long_summary}, {"type": "done"}]},
            {"expect": {"userContains": "next turn"},
             "chunks": [{"type": "text", "text": "reply b"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.begin_persistent().await;

    let out1 = h.turn_via_message("please run the heavy turn").await;
    assert!(
        out1.finished && out1.text.contains("reply a"),
        "turn 1: {out1:?}"
    );

    let out2 = h.turn_via_message("now the next turn").await;
    assert!(
        out2.finished && out2.text.contains("reply b"),
        "turn 2 must complete after auto-compaction; out: {out2:?}"
    );
    assert!(
        out2.events
            .iter()
            .any(|e| e.starts_with("Compacted") || e.starts_with("CompactProgress")),
        "usage past the window threshold must auto-compact at the turn \
         boundary; events: {:?}",
        out2.events
    );

    let journal = h.journal().await;
    assert!(
        journal.as_array().is_some_and(|calls| calls.len() >= 3),
        "turn 1 + summarization + turn 2 means at least three LLM calls; \
         journal: {journal}"
    );
}
