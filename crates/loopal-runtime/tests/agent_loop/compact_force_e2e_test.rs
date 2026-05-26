use loopal_context::ContextBudget;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ContentBlock, Message};
use loopal_test_support::{HarnessBuilder, chunks};

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

fn padded_user_msg(label: &str) -> Message {
    Message::user(&format!("{label}: {}", "x".repeat(100)))
}

fn padded_seed(n: usize) -> Vec<Message> {
    (0..n)
        .map(|i| padded_user_msg(&format!("msg-{i}")))
        .collect()
}

#[tokio::test]
async fn manual_compact_reduces_turns() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("summary")])
        .messages(padded_seed(15))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    let before = h.runner.turns.store().turns().len();
    h.runner.force_compact(None).await.unwrap();

    // boundary turn keeps last user turn; older are dropped via CompactionSummary
    // on the boundary. The view drops everything before the boundary turn.
    let view_len = h.runner.turns.view().len();
    assert!(
        view_len < before,
        "expected wire to shrink after compact: view={view_len}, before={before}"
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    assert!(
        evts.iter()
            .any(|e| matches!(e, AgentEventPayload::Compacted(_))),
        "expected Compacted event"
    );
}

#[tokio::test]
async fn compact_event_payload_carries_manual_strategy_label() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("summary")])
        .messages(padded_seed(15))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let stats = evts.iter().find_map(|e| match e {
        AgentEventPayload::Compacted(s) => Some(s.clone()),
        _ => None,
    });
    let stats = stats.expect("Compacted event must fire");
    assert!(stats.kept > 0);
    assert!(stats.removed > 0);
    assert!(
        stats.strategy.starts_with("manual"),
        "expected manual-* strategy, got {}",
        stats.strategy
    );
}

#[tokio::test]
async fn compact_preserves_most_recent_user_message() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("summary")])
        .messages(padded_seed(20))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner.force_compact(None).await.unwrap();

    let last_text = h.runner.turns.view().messages().iter().rev().find_map(|m| {
        m.content.iter().find_map(|b| match b {
            ContentBlock::Text { text } if text.starts_with("msg-") => Some(text.clone()),
            _ => None,
        })
    });
    let text = last_text.expect("expected at least one msg-* in projected view");
    assert!(
        text.starts_with("msg-19"),
        "most recent user turn must survive compact, got: {text:?}"
    );
}

#[tokio::test]
async fn force_compact_short_circuits_on_tiny_history() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .messages(vec![Message::user("only one")])
        .build()
        .await;

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let saw_compacted = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Compacted(_)));
    assert!(
        !saw_compacted,
        "Compacted must not fire when there's nothing to compact"
    );
}

#[tokio::test]
async fn force_compact_handles_empty_store() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .messages(vec![])
        .build()
        .await;

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    assert!(
        !evts
            .iter()
            .any(|e| matches!(e, AgentEventPayload::Compacted(_))),
        "Compacted must not fire on empty store"
    );
}
