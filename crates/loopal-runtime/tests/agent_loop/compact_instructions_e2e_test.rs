use loopal_context::ContextBudget;
use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_turn::{Turn, TurnTrigger};

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
        .map(|i| Message::user(&format!("m{i}: {}", "x".repeat(100))))
        .collect()
}

// smart_compact_llm builds a single-turn ChatParams; user prompt is in trigger.
fn first_user_text(turns: &[Turn]) -> String {
    turns
        .iter()
        .find_map(|t| match &t.trigger {
            TurnTrigger::UserInput { content, .. } => Some(content.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn force_compact_injects_custom_instructions_into_prompt() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .messages(padded_seed(6))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner
        .force_compact(Some("preserve test repro steps".into()))
        .await
        .unwrap();

    let calls = h.recorded_messages.lock().unwrap();
    let first_call = calls.first().expect("summarization LLM call missing");
    let prompt = first_user_text(first_call);

    assert!(prompt.contains("<custom-instructions>"));
    assert!(prompt.contains("preserve test repro steps"));
    assert!(prompt.contains("</custom-instructions>"));
}

#[tokio::test]
async fn force_compact_omits_custom_instructions_when_none() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .messages(padded_seed(6))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner.force_compact(None).await.unwrap();

    let calls = h.recorded_messages.lock().unwrap();
    let first_call = calls.first().expect("summarization LLM call missing");
    let prompt = first_user_text(first_call);

    assert!(
        !prompt.contains("<custom-instructions>"),
        "prompt must not emit empty <custom-instructions> tag, got: {prompt:?}"
    );
}

#[tokio::test]
async fn force_compact_treats_whitespace_instructions_as_absent() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .messages(padded_seed(6))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner
        .force_compact(Some("   \n  ".into()))
        .await
        .unwrap();

    let calls = h.recorded_messages.lock().unwrap();
    let first_call = calls.first().expect("summarization LLM call missing");
    let prompt = first_user_text(first_call);

    assert!(
        !prompt.contains("<custom-instructions>"),
        "whitespace-only instructions must collapse to absent, got: {prompt:?}"
    );
}
