use serde_json::json;

use crate::support::CliHarness;

/// A retryable 503 on the first attempt, success on the retry. The agent's real
/// retry/backoff path (exercised over the wire) must recover the turn.
#[tokio::test]
async fn agent_retries_a_503_and_recovers_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "retry_503",
        "calls": [
            {"expect": {"userContains": "retry me"}, "status": 503, "retryAfterMs": 10},
            {"expect": {"userContains": "retry me"},
             "chunks": [{"type": "text", "text": "recovered"}, {"type": "done"}]}
        ]
    }))
    .await;

    let out = h.run_turn("retry me").await;
    assert!(
        out.error.is_none(),
        "turn should recover from a 503, not error: {:?}\nevents: {:?}",
        out.error,
        out.events
    );
    assert!(
        out.finished,
        "turn did not finish; events: {:?}",
        out.events
    );
    assert!(
        out.text.contains("recovered"),
        "the retry's success text should appear; text: {:?}",
        out.text
    );
}

/// A `max_tokens` truncation on the first call must auto-continue into a second
/// call that finishes the turn — driven over the real wire.
#[tokio::test]
async fn agent_auto_continues_after_max_tokens_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "max_tokens_continue",
        "calls": [
            {"expect": {"userContains": "continue me"},
             "chunks": [{"type": "text", "text": "part one "}, {"type": "done", "reason": "max_tokens"}]},
            // The continuation is a fresh request whose last user text differs, so
            // match it with an empty expect rather than the original prompt.
            {"expect": {},
             "chunks": [{"type": "text", "text": "part two"}, {"type": "done"}]}
        ]
    }))
    .await;

    let out = h.run_turn("continue me").await;
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
    assert!(
        out.text.contains("part two"),
        "auto-continuation should reach the second call's text; text: {:?}",
        out.text
    );
}

/// A mid-stream disconnect (socket dropped after partial output, no terminal
/// event) must be recovered via the agent's truncation → auto-continue path —
/// validated over the real HTTP/SSE wire, not an in-process stream double.
#[tokio::test]
async fn agent_recovers_from_a_mid_stream_disconnect_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "stream_disconnect",
        "calls": [
            {"expect": {"userContains": "drop me"},
             "chunks": [{"type": "text", "text": "partial output "}, {"type": "disconnect"}]},
            {"expect": {},
             "chunks": [{"type": "text", "text": "recovered after drop"}, {"type": "done"}]}
        ]
    }))
    .await;

    let out = h.run_turn("drop me").await;
    assert!(
        out.finished,
        "should recover from the drop and finish; error: {:?}\nevents: {:?}",
        out.error, out.events
    );
    assert!(
        out.text.contains("recovered after drop"),
        "auto-continuation after the drop should complete the turn; text: {:?}",
        out.text
    );
}

/// A 401 is a fatal auth error — the agent must surface it, not silently succeed.
#[tokio::test]
async fn agent_surfaces_a_401_as_a_fatal_error() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "fatal_401",
        "calls": [{"expect": {"userContains": "who am i"}, "status": 401}],
        "fallback": {"status": 401}
    }))
    .await;

    let out = h.run_turn("who am i").await;
    assert!(
        out.error.is_some() || !out.finished,
        "a 401 should not produce a clean finished turn; outcome: {out:?}"
    );
}
