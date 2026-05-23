use loopal_context::ContextBudget;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ContentBlock, Message};
use loopal_test_support::{HarnessBuilder, chunks};

/// Drain all available events from the channel (non-blocking after brief yield).
/// Create a tiny budget so small messages trigger compaction.
/// message_budget=425, half=212. Each message ~30 tokens (120 chars / 4).
/// 15 messages × 30 tokens = 450 > 212 → token_aware_keep_count returns ~7.
fn tiny_budget() -> ContextBudget {
    ContextBudget {
        context_window: 500,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 50,
        safety_margin: 25,
        message_budget: 425,
        max_output_tokens: 50,
    }
}

/// Build a user message with enough text to be ~30 tokens.
fn padded_user_msg(label: &str) -> Message {
    Message::user(&format!("{label}: {}", "x".repeat(100)))
}

#[tokio::test]
async fn test_manual_compact_reduces_messages() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("summary")])
        .build()
        .await;

    // Use tiny budget so 15 small messages exceed 75% threshold.
    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..15 {
        h.runner
            .params
            .store
            .push_user(padded_user_msg(&format!("msg-{i}")));
    }

    h.runner.force_compact(None).await.unwrap();

    assert!(
        h.runner.params.store.len() <= 12,
        "expected <=12 after compact, got {}",
        h.runner.params.store.len()
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    assert!(
        evts.iter()
            .any(|e| matches!(e, AgentEventPayload::Compacted(_))),
        "expected Compacted event, got: {evts:?}"
    );
}

#[tokio::test]
async fn test_compact_emits_event_payload() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("summary")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..15 {
        h.runner
            .params
            .store
            .push_user(padded_user_msg(&format!("msg-{i}")));
    }

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let compacted = evts.iter().find_map(|e| match e {
        AgentEventPayload::Compacted(s) => Some((&s.kept, &s.removed, s.strategy.clone())),
        _ => None,
    });
    let (kept, removed, strategy) = compacted.expect("Compacted event missing");

    assert!(*kept > 0, "kept should be positive");
    assert!(*removed > 0, "removed should be positive");
    assert_eq!(kept + removed, 15);
    assert!(
        strategy.starts_with("manual"),
        "expected manual-* strategy, got {strategy}"
    );
}

#[tokio::test]
async fn test_compact_preserves_recent_messages() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("summary")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..20 {
        h.runner
            .params
            .store
            .push_user(padded_user_msg(&format!("msg-{i}")));
    }

    h.runner.force_compact(None).await.unwrap();

    let last_text = h.runner.params.store.messages().last().and_then(|m| {
        m.content.iter().find_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
    });
    assert!(
        last_text.as_deref().unwrap_or("").starts_with("msg-19"),
        "last message should be msg-19, got: {last_text:?}"
    );
    assert!(h.runner.params.store.len() <= 12);
}

#[tokio::test]
async fn force_compact_short_circuits_on_tiny_history() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.store.clear();
    h.runner.params.store.push_user(Message::user("only one"));

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let saw_nothing_to_compact = evts.iter().any(
        |e| matches!(e, AgentEventPayload::Stream { text } if text.contains("nothing to compact")),
    );
    let saw_compacted = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Compacted(_)));

    assert!(
        saw_nothing_to_compact,
        "expected Stream(\"[nothing to compact...]\"), got: {evts:?}",
    );
    assert!(
        !saw_compacted,
        "Compacted event must not fire when there is nothing to compact",
    );
}

#[tokio::test]
async fn force_compact_handles_empty_store() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.store.clear();

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    assert!(
        !evts
            .iter()
            .any(|e| matches!(e, AgentEventPayload::Compacted(_))),
        "Compacted must not fire for empty store: {evts:?}",
    );
}
