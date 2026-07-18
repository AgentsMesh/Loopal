use std::time::Duration;

use serde_json::json;

use crate::support::CliHarness;

/// A user message arriving while a turn is still streaming must be queued —
/// not lost, not interleaved — and consumed as its own turn right after the
/// in-flight one settles.
#[tokio::test]
async fn message_sent_mid_turn_queues_and_runs_next() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "queued_message",
        "calls": [
            {"expect": {"userContains": "slow first"},
             "chunks": [
                {"type": "delay", "ms": 1500},
                {"type": "text", "text": "slow done"},
                {"type": "done"}
             ]},
            {"expect": {"userContains": "queued second"},
             "chunks": [{"type": "text", "text": "second done"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.begin_persistent().await;

    h.message_fire("slow first please").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    h.message_fire("queued second please").await;

    let out1 = h.collect_persistent().await;
    assert!(
        out1.finished && out1.text.contains("slow done"),
        "the in-flight turn must complete undisturbed; out: {out1:?}"
    );
    assert!(
        !out1.text.contains("second done"),
        "the queued message must not interleave into the in-flight turn; \
         out: {out1:?}"
    );

    assert!(
        h.await_event("second done", Duration::from_secs(8)).await,
        "the queued message must run as its own turn after the first settles"
    );
}
