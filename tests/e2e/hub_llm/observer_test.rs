use std::time::Duration;

use serde_json::json;

use crate::support::HubHarness;

/// Multi-client session sharing: a second registered UI client must receive
/// the same broadcast turn events (stream text and completion) that the
/// driving client sees.
#[tokio::test]
async fn second_ui_client_observes_the_turn_stream() {
    let mut h = HubHarness::start(json!({
        "version": 2,
        "name": "observer",
        "calls": [
            {"expect": {"userContains": "observed turn"},
             "chunks": [{"type": "text", "text": "observed-answer"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    let mut observer = h.second_client("e2e-observer").await;

    let (out, seen) = tokio::join!(
        h.turn("please run the observed turn"),
        observer.collect_until_settled(Duration::from_secs(50)),
    );
    assert!(
        out.finished && out.text.contains("observed-answer"),
        "driving client turn failed: {out:?}"
    );
    assert!(
        seen.iter()
            .any(|e| e.starts_with("Stream") && e.contains("observed-answer")),
        "the observer must see the same streamed text; observer events: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|e| e.starts_with("Finished") || e.starts_with("AwaitingInput")),
        "the observer must see the turn settle; observer events: {seen:?}"
    );
}
