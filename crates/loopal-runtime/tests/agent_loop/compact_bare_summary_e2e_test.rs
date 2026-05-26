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

fn padded_seed(n: usize) -> Vec<Message> {
    (0..n)
        .map(|i| Message::user(&format!("msg-{i}: {}", "x".repeat(100))))
        .collect()
}

fn first_text(h: &loopal_test_support::IntegrationHarness) -> String {
    h.runner
        .turns
        .view()
        .messages()
        .first()
        .and_then(|m| {
            m.content.iter().find_map(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn compact_falls_back_to_bare_summary_on_llm_failure() {
    let mut h = HarnessBuilder::new()
        .calls(vec![vec![chunks::non_retryable_error("simulated 400")]])
        .messages(padded_seed(15))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    let before = h.runner.turns.view().len();
    h.runner.force_compact(None).await.unwrap();

    assert!(
        h.runner.turns.view().len() < before,
        "bare_summary fallback must still reduce wire size: before={before}, after={}",
        h.runner.turns.view().len()
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let summary = evts
        .iter()
        .find_map(|e| match e {
            AgentEventPayload::Compacted(s) => Some(s),
            _ => None,
        })
        .expect("Compacted event must fire even on LLM failure");
    assert!(summary.removed > 0);
    assert!(summary.kept > 0);
}

#[tokio::test]
async fn compact_bare_summary_persists_deterministic_outline() {
    let mut h = HarnessBuilder::new()
        .calls(vec![vec![chunks::non_retryable_error(
            "simulated 500-class",
        )]])
        .messages(padded_seed(15))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner.force_compact(None).await.unwrap();

    let text = first_text(&h);
    assert!(
        text.contains("Bare Summary") && text.contains("User turns:"),
        "expected deterministic bare_summary outline, got: {text:?}"
    );
}

#[tokio::test]
async fn compact_falls_back_to_bare_summary_on_empty_llm_response() {
    // LLM returns Ok(Text{""}) — distinct from the Err path. extract_summary
    // strips to "" and smart_compact MUST fall back to bare_summary rather
    // than persisting an empty summary message.
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("")])
        .messages(padded_seed(15))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    let before = h.runner.turns.view().len();
    h.runner.force_compact(None).await.unwrap();

    assert!(
        h.runner.turns.view().len() < before,
        "empty-response fallback must still reduce wire"
    );
    assert!(
        first_text(&h).contains("Bare Summary"),
        "empty LLM response must trigger bare_summary"
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    assert!(
        evts.iter()
            .any(|e| matches!(e, AgentEventPayload::Compacted(_))),
        "Compacted event must fire on empty-response fallback"
    );
}

#[tokio::test]
async fn compact_extracts_tagged_summary_from_llm_response() {
    // Prompt asks the model to wrap output in <summary>...</summary>.
    // The store must retain only the inner body — not the analysis
    // scratchpad or the tags themselves.
    let llm_response =
        "<analysis>\nfoo bar drafting\n</analysis>\n<summary>\nFINAL_SUMMARY_TEXT\n</summary>";
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn(llm_response)])
        .messages(padded_seed(15))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner.force_compact(None).await.unwrap();

    let text = first_text(&h);
    assert!(text.contains("FINAL_SUMMARY_TEXT"), "got: {text:?}");
    assert!(
        !text.contains("foo bar drafting"),
        "analysis scratchpad must NOT be persisted, got: {text:?}"
    );
    assert!(
        !text.contains("<summary>") && !text.contains("</summary>"),
        "tags must not survive, got: {text:?}"
    );
}
