use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::{API_KEY, CliHarness};

fn span_attr<'a>(span: &'a Value, key: &str) -> Option<&'a str> {
    span["attributes"].as_array()?.iter().find_map(|pair| {
        let pair = pair.as_array()?;
        (pair.first()?.as_str()? == key)
            .then(|| pair.get(1)?.as_str())
            .flatten()
    })
}

fn read_trace_spans(h: &CliHarness) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(h.telemetry_dir()) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("traces-") && name.ends_with(".jsonl"))
        })
        .flat_map(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        // The exporter may be appending the last line while it is read. Ignore
        // that partial line and retry below until every expected span arrives.
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

async fn wait_for_retry_trace_spans(h: &CliHarness) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let spans = read_trace_spans(h);
        let counts = spans
            .iter()
            .fold(HashMap::<&str, usize>::new(), |mut acc, span| {
                if let Some(name) = span["name"].as_str() {
                    *acc.entry(name).or_default() += 1;
                }
                acc
            });
        if counts.get("turn").copied().unwrap_or_default() >= 1
            && counts.get("provider_attempt").copied().unwrap_or_default() >= 2
            && counts.get("http_request").copied().unwrap_or_default() >= 2
        {
            return spans;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for exported retry trace spans; counts: {counts:?}; spans: {spans:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// A retryable 502 on the first attempt, success on the retry. This mirrors the
/// gateway failure seen in production and exercises the real HTTP adapter,
/// retry loop, and transient retry-state lifecycle together.
#[tokio::test]
async fn agent_retries_a_502_clears_retry_state_and_recovers_over_the_wire() {
    let mut h = CliHarness::start_with_telemetry(json!({
        "version": 2,
        "name": "retry_502",
        "calls": [
            {"expect": {"userContains": "retry me"}, "status": 502, "retryAfterMs": 10},
            {"expect": {"userContains": "retry me"},
             "chunks": [{"type": "text", "text": "recovered"}, {"type": "done"}]}
        ]
    }))
    .await;

    let out = h.run_turn("retry me").await;
    assert!(
        out.error.is_none(),
        "turn should recover from a 502, not error: {:?}\nevents: {:?}",
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

    let retry_error = out
        .events
        .iter()
        .position(|event| event.starts_with("RetryError"))
        .expect("a wire-level 502 must publish the transient retry state");
    let retry_cleared = out
        .events
        .iter()
        .position(|event| event.starts_with("RetryCleared"))
        .expect("a successful retry must clear the transient retry state");
    assert!(
        retry_error < retry_cleared,
        "RetryCleared must follow RetryError; events: {:?}",
        out.events
    );
    assert_eq!(
        out.events
            .iter()
            .filter(|event| event.starts_with("RetryError"))
            .count(),
        1,
        "the single 502 response should schedule exactly one retry; events: {:?}",
        out.events
    );
    assert!(
        out.events
            .iter()
            .any(|event| event.starts_with("RetryError") && event.contains("Retrying in 0.0s")),
        "the 10ms Retry-After must survive the 502 adapter path; events: {:?}",
        out.events
    );
    assert_eq!(
        out.events
            .iter()
            .filter(|event| event.starts_with("RetryCleared"))
            .count(),
        1,
        "the retry lifecycle must have exactly one terminal clear; events: {:?}",
        out.events
    );

    let verify = h.verify().await;
    assert_eq!(verify["served"], 2, "502 + retry must make two wire calls");
    assert_eq!(verify["remaining"], 0);
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");

    let spans = wait_for_retry_trace_spans(&h).await;
    let turn = spans
        .iter()
        .find(|span| {
            span["name"] == "turn"
                && span_attr(span, "gen_ai.request.model") == Some("claude-opus-4-8")
        })
        .expect("completed agent turn span");
    let turn_uuid = span_attr(turn, "loopal.turn.uuid").expect("stable journal turn UUID");
    uuid::Uuid::parse_str(turn_uuid).expect("loopal.turn.uuid must be a raw UUID");
    let trace_id = turn["trace_id"].as_str().expect("turn trace id");
    let turn_span_id = turn["span_id"].as_str().expect("turn span id");

    let mut attempts = spans
        .iter()
        .filter(|span| span["name"] == "provider_attempt" && span["trace_id"] == trace_id)
        .collect::<Vec<_>>();
    attempts.sort_by_key(|span| {
        span_attr(span, "loopal.retry.attempt")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_default()
    });
    assert_eq!(attempts.len(), 2, "502 + recovery must export two attempts");
    for (index, attempt) in attempts.iter().enumerate() {
        assert_eq!(attempt["parent_span_id"], turn_span_id);
        assert_eq!(span_attr(attempt, "loopal.provider.phase"), Some("main"));
        assert_eq!(
            span_attr(attempt, "loopal.retry.attempt"),
            Some((index + 1).to_string().as_str())
        );
        assert_eq!(span_attr(attempt, "loopal.retry.max_retries"), Some("6"));
        assert_eq!(span_attr(attempt, "gen_ai.system"), Some("anthropic"));
        assert_eq!(
            span_attr(attempt, "gen_ai.request.model"),
            Some("claude-opus-4-8")
        );
    }
    assert_eq!(attempts[0]["status"], "error");
    assert_eq!(span_attr(attempts[0], "error.type"), Some("provider"));
    assert_eq!(attempts[1]["status"], "unset");

    for (index, attempt) in attempts.iter().enumerate() {
        let attempt_span_id = attempt["span_id"].as_str().expect("attempt span id");
        let http = spans
            .iter()
            .find(|span| {
                span["name"] == "http_request"
                    && span["trace_id"] == trace_id
                    && span["parent_span_id"] == attempt_span_id
            })
            .expect("HTTP span parented to provider attempt");
        assert_eq!(http["kind"], "client");
        assert_eq!(span_attr(http, "http.request.method"), Some("POST"));
        assert_eq!(span_attr(http, "server.address"), Some("127.0.0.1"));
        assert_eq!(span_attr(http, "gen_ai.system"), Some("anthropic"));
        let expected_status = if index == 0 { "502" } else { "200" };
        assert_eq!(
            span_attr(http, "http.response.status_code"),
            Some(expected_status)
        );
        if index == 0 {
            assert_eq!(http["status"], "error");
            assert_eq!(span_attr(http, "error.type"), Some("502"));
        } else {
            assert_eq!(http["status"], "unset");
            assert_eq!(span_attr(http, "error.type"), None);
        }

        let attributes = serde_json::to_string(&http["attributes"]).unwrap();
        assert!(!attributes.contains(API_KEY), "HTTP span leaked API key");
        assert!(
            !attributes.contains(&h.base_url),
            "HTTP span must not export endpoint/port: {attributes}"
        );
        assert!(
            !attributes.contains("/v1/messages"),
            "HTTP span must not export path: {attributes}"
        );
    }
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
            {"expect": {"bodyContains": "partial output"},
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
    assert!(
        !out.events
            .iter()
            .any(|event| event.starts_with("RetryError")),
        "partial output must continue with context, not enter exact-request retry: {:?}",
        out.events
    );

    let verify = h.verify().await;
    assert_eq!(verify["served"], 2, "disconnect + continuation: {verify}");
    assert_eq!(verify["remaining"], 0, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");

    let journal = h.journal().await;
    assert_eq!(
        journal.as_array().map(Vec::len),
        Some(2),
        "expected the original request and one continuation: {journal}"
    );
    let first_message_count = journal[0]["messageCount"].as_u64().unwrap_or(0);
    let continuation_message_count = journal[1]["messageCount"].as_u64().unwrap_or(0);
    assert!(
        continuation_message_count > first_message_count,
        "continuation must append partial assistant context instead of replaying the original request: {journal}"
    );
    assert!(
        journal[1]["assistantBlockTypes"]
            .as_array()
            .is_some_and(|types| types.iter().any(|value| value == "text")),
        "continuation must carry the partial assistant text block: {journal}"
    );
    assert_eq!(
        journal[1]["matched"], true,
        "the second request must contain the partial output required by the mock expectation: {journal}"
    );
}

/// A tool call is provisional until the surrounding model response reaches a
/// valid terminal marker. If the wire drops first, the UI must receive a
/// terminal stale result before continuation so its working indicator cannot
/// remain active for a tool the runtime deliberately discarded.
#[tokio::test]
async fn discarded_tool_call_is_terminalized_before_wire_recovery_continues() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "discard_incomplete_tool",
        "calls": [
            {"expect": {"userContains": "provisional tool"},
             "chunks": [
                 {"type": "text", "text": "checking "},
                 {"type": "tool_use", "id": "discard-me", "name": "Read",
                  "input": {"file_path": "/tmp/not-executed"}},
                 {"type": "disconnect"}
             ]},
            {"expect": {"bodyContains": "checking"},
             "chunks": [
                 {"type": "text", "text": "recovered without executing it"},
                 {"type": "done"}
             ]}
        ]
    }))
    .await;

    let out = h.run_turn("provisional tool").await;
    assert!(out.error.is_none(), "outcome: {out:?}");
    assert!(out.finished, "outcome: {out:?}");

    let tool_call = out
        .events
        .iter()
        .position(|event| event.starts_with("ToolCall") && event.contains("discard-me"))
        .expect("provisional ToolCall event");
    let discarded = out
        .events
        .iter()
        .position(|event| {
            event.starts_with("ToolResult")
                && event.contains("discard-me")
                && event.contains("IncompleteModelResponse")
        })
        .expect("terminal stale ToolResult for discarded call");
    let continuation = out
        .events
        .iter()
        .position(|event| event.starts_with("AutoContinuation"))
        .expect("continuation event");
    assert!(
        tool_call < discarded && discarded < continuation,
        "discard must be terminal before continuation: {:?}",
        out.events
    );
    assert_eq!(out.tool_result_count(), 1, "events: {:?}", out.events);

    let verify = h.verify().await;
    assert_eq!(verify["served"], 2, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}

#[tokio::test]
async fn discarded_server_tool_is_terminalized_before_wire_recovery_continues() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "discard_incomplete_server_tool",
        "calls": [
            {"expect": {"userContains": "provisional search"},
             "chunks": [
                 {"type": "text", "text": "searching "},
                 {"type": "server_tool_use", "id": "server-discard-me",
                  "name": "web_search", "input": {"query": "rust"}},
                 {"type": "disconnect"}
             ]},
            {"expect": {"bodyContains": "searching"},
             "chunks": [
                 {"type": "text", "text": "recovered after search drop"},
                 {"type": "done"}
             ]}
        ]
    }))
    .await;

    let out = h.run_turn("provisional search").await;
    assert!(out.error.is_none(), "outcome: {out:?}");
    assert!(out.finished, "outcome: {out:?}");

    let tool_use = out
        .events
        .iter()
        .position(|event| event.starts_with("ServerToolUse") && event.contains("server-discard-me"))
        .expect("provisional ServerToolUse event");
    let discarded = out
        .events
        .iter()
        .position(|event| {
            event.starts_with("ServerToolDiscarded")
                && event.contains("server-discard-me")
                && event.contains("IncompleteModelResponse")
        })
        .expect("terminal ServerToolDiscarded event");
    let continuation = out
        .events
        .iter()
        .position(|event| event.starts_with("AutoContinuation"))
        .expect("continuation event");
    assert!(
        tool_use < discarded && discarded < continuation,
        "discard must be terminal before continuation: {:?}",
        out.events
    );

    let verify = h.verify().await;
    assert_eq!(verify["served"], 2, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}

/// Repeated partial HTTP/SSE disconnects are not a successful answer. The
/// runtime may preserve each fragment and issue bounded continuation requests,
/// but exhausting that budget must terminate as Error rather than promote the
/// last unterminated fragment to Goal.
#[tokio::test]
async fn agent_errors_when_partial_stream_continuations_never_reach_done() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "partial_stream_continuation_exhaustion",
        "calls": [
            {"expect": {"userContains": "never finish"},
             "chunks": [{"type": "text", "text": "fragment one"}, {"type": "disconnect"}]},
            {"expect": {"bodyContains": "fragment one"},
             "chunks": [{"type": "text", "text": "fragment two"}, {"type": "disconnect"}]},
            {"expect": {"bodyContains": "fragment two"},
             "chunks": [{"type": "text", "text": "fragment three"}, {"type": "disconnect"}]},
            {"expect": {"bodyContains": "fragment three"},
             "chunks": [{"type": "text", "text": "fragment four"}, {"type": "disconnect"}]}
        ]
    }))
    .await;

    let out = h.run_turn("never finish").await;
    assert!(out.error.is_some(), "outcome: {out:?}");
    assert!(
        !out.finished,
        "an unterminated response cannot finish as Goal"
    );
    assert!(
        out.text.contains("fragment four"),
        "caller-visible partial output must be preserved; outcome: {out:?}"
    );
    assert_eq!(
        out.events
            .iter()
            .filter(|event| event.starts_with("AutoContinuation"))
            .count(),
        3,
        "continuation budget must stay bounded; events: {:?}",
        out.events
    );
    assert!(
        !out.events
            .iter()
            .any(|event| event.starts_with("RetryError")),
        "partial responses use contextual continuation, not exact replay: {:?}",
        out.events
    );

    let verify = h.verify().await;
    assert_eq!(verify["served"], 4, "mock scenario: {verify}");
    assert_eq!(verify["remaining"], 0, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}

/// An HTTP 200 followed by zero response-body bytes is not a valid completed
/// attempt. Because no semantic output escaped, the exact request is replayed
/// within the retry budget rather than converted into an empty Goal.
#[tokio::test]
async fn agent_retries_zero_byte_stream_eof_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "zero_byte_eof",
        "calls": [
            {"expect": {"userContains": "empty body"}, "disconnectAfterEvents": 0},
            {"expect": {"userContains": "empty body"},
             "chunks": [{"type": "text", "text": "recovered from empty body"}, {"type": "done"}]}
        ]
    }))
    .await;

    let out = h.run_turn("empty body").await;
    assert!(out.error.is_none(), "events: {:?}", out.events);
    assert!(out.finished, "events: {:?}", out.events);
    assert!(out.text.contains("recovered from empty body"));
    assert_eq!(
        out.events
            .iter()
            .filter(|event| event.starts_with("RetryError"))
            .count(),
        1
    );
    let verify = h.verify().await;
    assert_eq!(verify["served"], 2);
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}

/// Anthropic can send a retryable error inside an already-established SSE
/// response. The full attempt span must fail and the request is safe to replay
/// when that error arrived before model output.
#[tokio::test]
async fn agent_retries_in_stream_overload_and_marks_attempt_failed() {
    let mut h = CliHarness::start_with_telemetry(json!({
        "version": 2,
        "name": "in_stream_overload",
        "calls": [
            {"expect": {"userContains": "overload in stream"},
             "rawSse": ["{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"busy\"}}"]},
            {"expect": {"userContains": "overload in stream"},
             "chunks": [{"type": "text", "text": "recovered from overload"}, {"type": "done"}]}
        ]
    }))
    .await;

    let out = h.run_turn("overload in stream").await;
    assert!(out.error.is_none(), "events: {:?}", out.events);
    assert!(out.finished, "events: {:?}", out.events);
    assert!(out.text.contains("recovered from overload"));

    let spans = wait_for_retry_trace_spans(&h).await;
    assert!(
        spans.iter().any(|span| {
            span["name"] == "provider_attempt"
                && span["status"] == "error"
                && span_attr(span, "error.type") == Some("stream")
        }),
        "the HTTP-200 stream failure must mark its provider attempt: {spans:?}"
    );
}

/// Six retries means seven provider attempts. Exhausting retryable 504s must
/// publish one balanced clear and terminate as Error, never as an empty Goal.
#[tokio::test]
async fn agent_surfaces_retry_exhaustion_as_error_over_the_wire() {
    let calls = (0..7)
        .map(|_| {
            json!({
                "expect": {"userContains": "exhaust gateway"},
                "status": 504,
                "retryAfterMs": 1
            })
        })
        .collect::<Vec<_>>();
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "exhaust_504",
        "calls": calls
    }))
    .await;

    let out = h.run_turn("exhaust gateway").await;
    assert!(out.error.is_some(), "outcome: {out:?}");
    assert!(!out.finished, "an exhausted request cannot finish as Goal");
    assert_eq!(
        out.events
            .iter()
            .filter(|event| event.starts_with("RetryError"))
            .count(),
        6,
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
    assert_eq!(verify["served"], 7);
    assert_eq!(verify["remaining"], 0);
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
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
