use loopal_context::ContextBudget;
use loopal_message::{ContentBlock, Message};
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

fn padded_user(label: &str) -> Message {
    Message::user(&format!("{label}: {}", "x".repeat(100)))
}

fn first_user_text(msgs: &[Message]) -> String {
    msgs.iter()
        .flat_map(|m| m.content.iter())
        .find_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn force_compact_injects_custom_instructions_into_prompt() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..6 {
        h.runner
            .params
            .store
            .push_user(padded_user(&format!("m{i}")));
    }

    h.runner
        .force_compact(Some("preserve test repro steps".into()))
        .await
        .unwrap();

    let calls = h.recorded_messages.lock().unwrap();
    let first_call = calls.first().expect("summarization LLM call missing");
    let prompt = first_user_text(first_call);

    assert!(
        prompt.contains("<custom-instructions>"),
        "prompt should open <custom-instructions> tag, got: {prompt:?}",
    );
    assert!(
        prompt.contains("preserve test repro steps"),
        "prompt should include user instructions verbatim, got: {prompt:?}",
    );
    assert!(
        prompt.contains("</custom-instructions>"),
        "prompt should close </custom-instructions> tag, got: {prompt:?}",
    );
}

#[tokio::test]
async fn force_compact_omits_custom_instructions_when_none() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..6 {
        h.runner
            .params
            .store
            .push_user(padded_user(&format!("m{i}")));
    }

    h.runner.force_compact(None).await.unwrap();

    let calls = h.recorded_messages.lock().unwrap();
    let first_call = calls.first().expect("summarization LLM call missing");
    let prompt = first_user_text(first_call);

    assert!(
        !prompt.contains("<custom-instructions>"),
        "prompt must not emit empty <custom-instructions> tag, got: {prompt:?}",
    );
}

#[tokio::test]
async fn force_compact_treats_whitespace_instructions_as_absent() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..6 {
        h.runner
            .params
            .store
            .push_user(padded_user(&format!("m{i}")));
    }

    h.runner
        .force_compact(Some("   \n  ".into()))
        .await
        .unwrap();

    let calls = h.recorded_messages.lock().unwrap();
    let first_call = calls.first().expect("summarization LLM call missing");
    let prompt = first_user_text(first_call);

    assert!(
        !prompt.contains("<custom-instructions>"),
        "whitespace-only instructions must collapse to absent, got: {prompt:?}",
    );
}
