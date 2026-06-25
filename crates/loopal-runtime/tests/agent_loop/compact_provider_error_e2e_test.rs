use loopal_protocol::AgentEventPayload;
use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, chunks};

fn five_user_messages() -> Vec<Message> {
    (1..=5)
        .map(|i| Message::user(&format!("turn {i} content")))
        .collect()
}

fn saw_unavailable_notice(evts: &[AgentEventPayload]) -> bool {
    evts.iter().any(|e| {
        matches!(e, AgentEventPayload::Stream { text }
            if text.contains("compaction unavailable"))
    })
}

#[tokio::test]
async fn force_compact_surfaces_inline_notice_when_summarization_provider_missing() {
    // Main model resolves (anthropic mock), but the summarization route points
    // at a GPT model whose provider isn't registered → the resolve fails and
    // must surface a visible inline notice (Stream), never a terminal Error.
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("unused")])
        .summarization_model("gpt-4o")
        .messages(five_user_messages())
        .build()
        .await;

    let result = h.runner.force_compact(None).await;
    assert!(
        matches!(result, Ok(false)),
        "missing summarization provider must yield Ok(false), got {result:?}"
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    assert!(
        saw_unavailable_notice(&evts),
        "provider-unresolved compaction must surface an inline notice; got: {evts:?}"
    );
    // Never the termination-semantic Error (would snap a viewed sub-agent to root).
    assert!(
        !evts
            .iter()
            .any(|e| matches!(e, AgentEventPayload::Error { .. })),
        "must NOT emit AgentEventPayload::Error; got: {evts:?}"
    );
}

#[tokio::test]
async fn force_compact_benign_decline_emits_no_unavailable_notice() {
    // A single-turn conversation bails before any LLM resolve — no notice.
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("unused")])
        .messages(vec![Message::user("only message")])
        .build()
        .await;

    let _ = h.runner.force_compact(None).await;

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    assert!(
        !saw_unavailable_notice(&evts),
        "benign short-conversation decline must not surface an unavailable notice; got: {evts:?}"
    );
}
