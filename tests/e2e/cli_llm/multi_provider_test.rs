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
