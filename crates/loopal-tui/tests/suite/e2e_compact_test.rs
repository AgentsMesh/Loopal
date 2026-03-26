//! Integration tests for compaction: manual compact, event payload, message preservation.

use loopal_context::ContextBudget;
use loopal_message::{ContentBlock, Message};
use loopal_protocol::AgentEventPayload;
use loopal_test_support::{HarnessBuilder, chunks};

/// Drain all available events from the channel (non-blocking after brief yield).
async fn drain_events(
    rx: &mut tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
) -> Vec<AgentEventPayload> {
    tokio::task::yield_now().await;
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        out.push(ev.payload);
    }
    out
}

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
    }
}

/// Build a user message with enough text to be ~30 tokens.
fn padded_user_msg(label: &str) -> Message {
    Message::user(&format!("{label}: {}", "x".repeat(100)))
}

/// `/compact` command reduces message count and emits Compacted event.
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

    h.runner.force_compact().await.unwrap();

    assert!(
        h.runner.params.store.len() <= 12,
        "expected <=12 after compact, got {}",
        h.runner.params.store.len()
    );

    let evts = drain_events(&mut h.event_rx).await;
    assert!(
        evts.iter()
            .any(|e| matches!(e, AgentEventPayload::Compacted { .. })),
        "expected Compacted event, got: {evts:?}"
    );
}

/// Compacted event carries correct payload fields.
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

    h.runner.force_compact().await.unwrap();

    let evts = drain_events(&mut h.event_rx).await;
    let compacted = evts.iter().find_map(|e| match e {
        AgentEventPayload::Compacted {
            kept,
            removed,
            strategy,
            ..
        } => Some((kept, removed, strategy.clone())),
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

/// Compaction preserves the most recent messages.
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

    h.runner.force_compact().await.unwrap();

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
