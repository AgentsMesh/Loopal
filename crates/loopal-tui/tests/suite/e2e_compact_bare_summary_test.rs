use loopal_context::ContextBudget;
use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
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

#[tokio::test]
async fn compact_falls_back_to_bare_summary_on_llm_failure() {
    let mut h = HarnessBuilder::new()
        .calls(vec![vec![chunks::non_retryable_error("simulated 400")]])
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
    let before = h.runner.params.store.len();

    h.runner.force_compact(None).await.unwrap();

    assert!(
        h.runner.params.store.len() < before,
        "bare_summary fallback must still reduce messages (before={before}, after={})",
        h.runner.params.store.len()
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let compacted = evts.iter().find_map(|e| match e {
        AgentEventPayload::Compacted(s) => Some(s),
        _ => None,
    });
    let summary = compacted.expect("Compacted event must fire even on LLM failure");

    assert!(
        summary.summary_msg_id.is_some(),
        "boundary marker must be anchored to a persisted summary id, got None"
    );
    assert!(summary.removed > 0, "removed count must be positive");
    assert!(summary.kept > 0, "kept count must be positive");
}

#[tokio::test]
async fn compact_bare_summary_persists_deterministic_outline() {
    let mut h = HarnessBuilder::new()
        .calls(vec![vec![chunks::non_retryable_error(
            "simulated 500-class",
        )]])
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

    let first_msg_text = h
        .runner
        .params
        .store
        .messages()
        .first()
        .and_then(|m| {
            m.content.iter().find_map(|b| match b {
                loopal_message::ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();

    assert!(
        first_msg_text.contains("Bare Summary") && first_msg_text.contains("User turns:"),
        "expected deterministic bare_summary outline, got: {first_msg_text:?}"
    );
}
