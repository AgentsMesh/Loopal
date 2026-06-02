use loopal_protocol::AgentEventPayload;
use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, chunks};

#[tokio::test]
async fn force_compact_emits_token_usage_matching_tokens_after() {
    let calls = vec![chunks::text_turn("<summary>summary</summary>")];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![
            Message::user("turn 1 content"),
            Message::user("turn 2 content"),
            Message::user("turn 3 content"),
            Message::user("turn 4 content"),
            Message::user("turn 5 content"),
        ])
        .build()
        .await;

    let _ = h.runner.force_compact(None).await;

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let tokens_after = evts
        .iter()
        .find_map(|e| match e {
            AgentEventPayload::Compacted(s) => Some(s.tokens_after),
            _ => None,
        })
        .expect("force_compact must emit Compacted");
    let usage_input = evts
        .iter()
        .find_map(|e| match e {
            AgentEventPayload::TokenUsage { input_tokens, .. } => Some(*input_tokens),
            _ => None,
        })
        .expect("force_compact must emit TokenUsage to refresh the ctx counter");

    assert_eq!(
        usage_input, tokens_after,
        "post-compact TokenUsage.input_tokens must equal Compacted.tokens_after \
         so the status bar ctx counter reflects the compacted size",
    );
}
