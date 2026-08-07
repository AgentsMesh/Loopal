use serde_json::json;

use crate::support::{CliHarness, Provider};

/// One semantic scenario, driven end-to-end through a different production
/// provider adapter each time. Proves the agent behaviour is provider-agnostic
/// and that each adapter reaches the mock over its own wire protocol.
async fn run_basic_over(provider: Provider, expected_protocol: &str) {
    let mut h = CliHarness::start_with(
        json!({
            "version": 2,
            "name": "multi_provider",
            "calls": [{
                "expect": {"userContains": "hello there"},
                "chunks": [
                    {"type": "text", "text": "greetings"},
                    {"type": "usage", "input": 4, "output": 2},
                    {"type": "done"}
                ]
            }],
            "fallback": {"chunks": [{"type": "text", "text": "ok"}, {"type": "done"}]}
        }),
        provider,
    )
    .await;

    let out = h.run_turn("hello there").await;
    assert!(
        out.error.is_none(),
        "{provider:?} errored: {:?}\nevents: {:?}",
        out.error,
        out.events
    );
    assert!(
        out.finished,
        "{provider:?} did not finish; events: {:?}",
        out.events
    );
    assert!(
        out.text.contains("greetings"),
        "{provider:?} text: {:?}",
        out.text
    );

    let journal = h.journal().await;
    assert_eq!(
        journal[0]["protocol"], expected_protocol,
        "{provider:?} should reach the mock over its own wire; journal: {journal}"
    );
}

#[tokio::test]
async fn google_provider_reaches_the_mock_over_the_wire() {
    run_basic_over(Provider::Google, "google").await;
}

#[tokio::test]
async fn openai_responses_provider_reaches_the_mock_over_the_wire() {
    run_basic_over(Provider::OpenAiResponses, "openai_responses").await;
}

#[tokio::test]
async fn openai_compat_provider_reaches_the_mock_over_the_wire() {
    run_basic_over(Provider::OpenAiCompat, "openai_compat").await;
}

#[tokio::test]
async fn openai_responses_recovers_from_502_exhaustion_and_accepts_the_next_turn() {
    let mut calls = vec![
        json!({
            "expect": {"protocol": "openai_responses", "userContains": "recover gateway"},
            "status": 502,
            "retryAfterMs": 1
        }),
        json!({
            "expect": {"protocol": "openai_responses", "userContains": "recover gateway"},
            "chunks": [
                {"type": "text", "text": "OpenAI recovered once."},
                {"type": "done"}
            ]
        }),
    ];
    calls.extend((0..7).map(|_| {
        json!({
            "expect": {"protocol": "openai_responses", "userContains": "exhaust gateway"},
            "status": 502,
            "retryAfterMs": 1
        })
    }));
    calls.push(json!({
        "expect": {"protocol": "openai_responses", "userContains": "recover next turn"},
        "chunks": [
            {"type": "text", "text": "OpenAI remained usable."},
            {"type": "done"}
        ]
    }));

    let mut h = CliHarness::start_with(
        json!({
            "version": 2,
            "name": "openai_responses_502_lifecycle",
            "calls": calls
        }),
        Provider::OpenAiResponses,
    )
    .await;
    h.begin_persistent().await;

    let recovered = h.turn_via_message("recover gateway").await;
    assert!(recovered.error.is_none(), "outcome: {recovered:?}");
    assert!(recovered.finished, "outcome: {recovered:?}");
    assert_eq!(recovered.text, "OpenAI recovered once.");
    assert_eq!(
        recovered
            .events
            .iter()
            .filter(|event| event.starts_with("RetryError"))
            .count(),
        1,
        "events: {:?}",
        recovered.events
    );
    assert_eq!(
        recovered
            .events
            .iter()
            .filter(|event| event.starts_with("RetryCleared"))
            .count(),
        1,
        "events: {:?}",
        recovered.events
    );

    let exhausted = h.turn_via_message("exhaust gateway").await;
    assert!(exhausted.error.is_some(), "outcome: {exhausted:?}");
    assert!(
        !exhausted.finished,
        "retry exhaustion cannot become an empty Goal: {exhausted:?}"
    );
    assert_eq!(
        exhausted
            .events
            .iter()
            .filter(|event| event.starts_with("RetryError"))
            .count(),
        6,
        "events: {:?}",
        exhausted.events
    );
    assert_eq!(
        exhausted
            .events
            .iter()
            .filter(|event| event.starts_with("RetryCleared"))
            .count(),
        1,
        "events: {:?}",
        exhausted.events
    );

    let next = h.turn_via_message("recover next turn").await;
    assert!(next.error.is_none(), "outcome: {next:?}");
    assert!(next.finished, "outcome: {next:?}");
    assert_eq!(next.text, "OpenAI remained usable.");

    let journal = h.journal().await;
    assert_eq!(
        journal.as_array().map(Vec::len),
        Some(10),
        "journal: {journal}"
    );
    assert!(
        journal
            .as_array()
            .is_some_and(|requests| requests.iter().all(|request| {
                request["protocol"] == "openai_responses" && request["matched"] == true
            })),
        "every request must traverse the OpenAI Responses adapter: {journal}"
    );
    let verify = h.verify().await;
    assert_eq!(verify["remaining"], 0, "mock scenario: {verify}");
    assert_eq!(verify["verified"], true, "mock scenario: {verify}");
}

#[tokio::test]
async fn google_prompt_block_cannot_complete_as_an_empty_goal() {
    let mut h = CliHarness::start_with(
        json!({
            "version": 2,
            "name": "google_prompt_block",
            "calls": [{
                "expect": {"protocol": "google", "userContains": "blocked prompt"},
                "rawSse": [
                    "{\"promptFeedback\":{\"blockReason\":\"SAFETY\",\"blockReasonMessage\":\"request blocked\"}}"
                ]
            }]
        }),
        Provider::Google,
    )
    .await;

    let out = h.run_turn("blocked prompt").await;
    assert!(out.text.is_empty(), "outcome: {out:?}");
    assert!(
        out.error
            .as_deref()
            .is_some_and(|error| error.contains("SAFETY")),
        "outcome: {out:?}"
    );
    assert!(
        !out.finished,
        "a blocked response cannot become Goal: {out:?}"
    );

    let journal = h.journal().await;
    assert_eq!(
        journal.as_array().map(Vec::len),
        Some(1),
        "journal: {journal}"
    );
    assert_eq!(journal[0]["protocol"], "google");
    assert_eq!(journal[0]["matched"], true);
}

#[tokio::test]
async fn google_partial_block_preserves_text_but_completes_as_error() {
    let mut h = CliHarness::start_with(
        json!({
            "version": 2,
            "name": "google_partial_block",
            "calls": [{
                "expect": {"protocol": "google", "userContains": "partial block"},
                "rawSse": [
                    "{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial findings\"}]}}]}",
                    "{\"candidates\":[{\"finishReason\":\"SAFETY\",\"finishMessage\":\"response blocked\"}]}"
                ]
            }]
        }),
        Provider::Google,
    )
    .await;

    let out = h.run_turn("partial block").await;
    assert_eq!(out.text, "partial findings", "outcome: {out:?}");
    assert!(
        out.error
            .as_deref()
            .is_some_and(|error| error.contains("SAFETY")),
        "outcome: {out:?}"
    );
    assert!(
        !out.finished,
        "a blocked response cannot become Goal: {out:?}"
    );

    let journal = h.journal().await;
    assert_eq!(
        journal.as_array().map(Vec::len),
        Some(1),
        "journal: {journal}"
    );
    assert_eq!(journal[0]["protocol"], "google");
    assert_eq!(journal[0]["matched"], true);
}
