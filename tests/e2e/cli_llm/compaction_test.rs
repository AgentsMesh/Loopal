use std::time::Duration;

use serde_json::json;

use crate::support::CliHarness;

fn compact_phases(events: &[String]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|event| {
            if !event.starts_with("CompactProgress") {
                return None;
            }
            if event.contains("phase: Summarize") {
                Some("summarize")
            } else if event.contains("phase: Rehydrate") {
                Some("rehydrate")
            } else if event.contains("phase: Done") {
                Some("done")
            } else {
                Some("unknown")
            }
        })
        .collect()
}

/// After several turns of history, manual compaction retries a real HTTP 502,
/// completes its progress lifecycle, and leaves the following normal turn free
/// of stale compaction or retry state.
#[tokio::test]
async fn agent_compacts_after_a_502_and_clears_lifecycle_state_over_the_wire() {
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
            {"expect": {"userContains": "gamma"}, "chunks": [{"type": "text", "text": "reply c"}, {"type": "done"}]},
            {"expect": {}, "status": 502, "retryAfterMs": 10},
            {"expect": {}, "chunks": [{"type": "text", "text": long_summary}, {"type": "done"}]},
            {"expect": {"userContains": "delta"}, "chunks": [{"type": "text", "text": "reply d"}, {"type": "done"}]}
        ]
    }))
    .await;
    h.begin_persistent().await;
    h.turn_via_message("alpha please").await;
    h.turn_via_message("beta please").await;
    h.turn_via_message("gamma please").await;

    let out = h.control(json!({"Compact": {"instructions": null}})).await;
    assert!(
        out.error.is_none(),
        "manual compaction should recover after retrying the 502; out: {out:?}"
    );
    assert_eq!(
        compact_phases(&out.events),
        vec!["summarize", "done"],
        "manual compaction must have a balanced terminal lifecycle; events: {:?}",
        out.events
    );
    let retry_error = out
        .events
        .iter()
        .position(|event| event.starts_with("RetryError"))
        .expect("the compaction 502 must publish retry state");
    let retry_cleared = out
        .events
        .iter()
        .position(|event| event.starts_with("RetryCleared"))
        .expect("successful compaction retry must clear retry state");
    let compacted = out
        .events
        .iter()
        .position(|event| event.starts_with("Compacted"))
        .expect("compaction must publish its structured result");
    let done = out
        .events
        .iter()
        .position(|event| event.starts_with("CompactProgress") && event.contains("phase: Done"))
        .expect("compaction must publish terminal Done");
    assert!(
        retry_error < retry_cleared && retry_cleared < compacted && compacted < done,
        "retry and compaction terminal events are out of order: {:?}",
        out.events
    );

    let next = h.turn_via_message("delta please").await;
    assert!(
        next.finished && next.error.is_none() && next.text.contains("reply d"),
        "normal work after compaction must complete; out: {next:?}"
    );
    assert!(
        compact_phases(&next.events).is_empty()
            && !next.events.iter().any(|event| {
                event.starts_with("RetryError") || event.starts_with("RetryCleared")
            }),
        "the following turn must not inherit stale compaction/retry state; events: {:?}",
        next.events
    );

    let verify = h.verify().await;
    assert_eq!(verify["served"], 6, "mock scenario: {verify}");
    assert_eq!(verify["remaining"], 0, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}

/// Compaction buffers its summary internally, so even a partial SSE response
/// is safe to discard and replay. It must never accept EOF-before-Done as a
/// valid summary or leave the retry/compaction lifecycle unbalanced.
#[tokio::test]
async fn agent_retries_truncated_compaction_stream_over_the_wire() {
    let long_summary = format!(
        "<summary>\n## Recovered summary\n{}\n</summary>",
        "recovered context ".repeat(80)
    );
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "compaction_stream_eof",
        "calls": [
            {"expect": {"userContains": "alpha"}, "chunks": [{"type": "text", "text": "reply a"}, {"type": "done"}]},
            {"expect": {"userContains": "beta"}, "chunks": [{"type": "text", "text": "reply b"}, {"type": "done"}]},
            {"expect": {"userContains": "gamma"}, "chunks": [{"type": "text", "text": "reply c"}, {"type": "done"}]},
            {"expect": {}, "chunks": [
                {"type": "text", "text": "<summary>TRUNCATED SUMMARY"},
                {"type": "disconnect"}
            ]},
            {"expect": {}, "chunks": [{"type": "text", "text": long_summary}, {"type": "done"}]}
        ]
    }))
    .await;
    h.begin_persistent().await;
    h.turn_via_message("alpha please").await;
    h.turn_via_message("beta please").await;
    h.turn_via_message("gamma please").await;

    let out = h.control(json!({"Compact": {"instructions": null}})).await;
    assert!(out.error.is_none(), "out: {out:?}");
    assert_eq!(compact_phases(&out.events), ["summarize", "done"]);
    assert_eq!(
        out.events
            .iter()
            .filter(|event| event.starts_with("RetryError"))
            .count(),
        1,
        "events: {:?}",
        out.events
    );
    assert_eq!(
        out.events
            .iter()
            .filter(|event| event.starts_with("RetryCleared"))
            .count(),
        1,
        "events: {:?}",
        out.events
    );

    let verify = h.verify().await;
    assert_eq!(verify["served"], 5, "mock scenario: {verify}");
    assert_eq!(verify["remaining"], 0, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}

/// Interrupting summarization is control flow, not a provider failure. A
/// cancelled pass must close its progress lifecycle without publishing
/// `Compacted` or replacing the prior conversation with a fallback summary.
#[tokio::test]
async fn cancelled_compaction_preserves_history_over_the_real_http_stack() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "cancel_compaction",
        "calls": [
            {"expect": {"userContains": "alpha"},
             "chunks": [{"type": "text", "text": "reply a"}, {"type": "done"}]},
            {"expect": {"userContains": "beta"},
             "chunks": [{"type": "text", "text": "reply b"}, {"type": "done"}]},
            {"expect": {"userContains": "gamma"},
             "chunks": [{"type": "text", "text": "reply c"}, {"type": "done"}]},
            {"expect": {},
             "chunks": [
                 {"type": "delay", "ms": 5000},
                 {"type": "text", "text": "<summary>must not commit</summary>"},
                 {"type": "done"}
             ]},
            {"expect": {"userContains": "delta", "bodyContains": "alpha please"},
             "chunks": [{"type": "text", "text": "history survived"}, {"type": "done"}]}
        ]
    }))
    .await;
    h.begin_persistent().await;
    h.turn_via_message("alpha please").await;
    h.turn_via_message("beta please").await;
    h.turn_via_message("gamma please").await;

    h.control_fire(json!({"Compact": {"instructions": null}}))
        .await;
    assert!(
        h.await_event("CompactProgress", Duration::from_secs(3))
            .await,
        "manual compaction must enter Summarize"
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let journal = h.journal().await;
        if journal.as_array().is_some_and(|calls| calls.len() >= 4) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "summarization request never reached the mock; journal: {journal}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    h.interrupt().await;
    let cancelled = h.collect_persistent().await;
    assert!(cancelled.error.is_none(), "cancel outcome: {cancelled:?}");
    assert!(
        cancelled
            .events
            .iter()
            .all(|event| !event.starts_with("Compacted")),
        "cancelled compaction rewrote history: {:?}",
        cancelled.events
    );
    assert!(
        cancelled
            .events
            .iter()
            .any(|event| { event.starts_with("CompactProgress") && event.contains("phase: Done") }),
        "cancelled compaction must close progress: {:?}",
        cancelled.events
    );

    let next = h.turn_via_message("delta please").await;
    assert!(
        next.finished && next.error.is_none() && next.text.contains("history survived"),
        "the full pre-cancel history must reach the next request: {next:?}"
    );
    let verify = h.verify().await;
    assert_eq!(verify["served"], 5, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}
