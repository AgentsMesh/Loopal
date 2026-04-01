//! Edge-case compaction tests: auto-compact on large context, thinking block stripping.

use loopal_context::ContextBudget;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::AgentEventPayload;
use loopal_test_support::{HarnessBuilder, chunks};

/// Build a large user message (~`n` estimated tokens via 4-bytes-per-token).
fn big_user_msg(label: &str, approx_tokens: usize) -> Message {
    let body = format!("{label}: {}", "x".repeat(approx_tokens * 4));
    Message::user(&body)
}

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

/// Auto-compaction fires when messages exceed 75% of a tiny context budget.
#[tokio::test]
async fn test_auto_compact_on_large_context() {
    // Two LLM calls: one for smart-compact summarization, one for the
    // actual turn after compaction succeeds.
    let mut h = HarnessBuilder::new()
        .calls(vec![
            chunks::text_turn("compact-summary"),
            chunks::text_turn("done"),
        ])
        .build()
        .await;

    // Shrink context budget so a handful of messages trigger compaction.
    h.runner.model_config.max_context_tokens = 500;
    h.runner.params.store.update_budget(tiny_budget());

    // Seed enough messages to exceed 75% of ~500-token budget.
    h.runner.params.store.clear();
    for i in 0..20 {
        h.runner
            .params
            .store
            .push_user(big_user_msg(&format!("m{i}"), 50));
    }
    let before = h.runner.params.store.len();

    let _ = h.runner.run().await;

    // After auto-compact + turn execution, message count should have decreased.
    // The run adds an assistant message, but compaction should have removed many.
    assert!(
        h.runner.params.store.len() < before,
        "expected fewer messages after auto-compact, before={before} after={}",
        h.runner.params.store.len()
    );

    let evts = drain_events(&mut h.event_rx).await;
    let has_compacted = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Compacted { .. }));
    assert!(has_compacted, "expected Compacted event from auto-compact");
}

/// `store.prepare_for_llm()` strips thinking blocks from old assistant messages
/// but preserves thinking in the last assistant message.
#[tokio::test]
async fn test_thinking_blocks_stripped_in_context_prep() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("ok")])
        .build()
        .await;

    // Build a conversation with two assistant messages containing thinking.
    let mut runner = h.runner;
    runner.params.store.clear();
    runner.params.store.push_user(Message::user("q1"));
    runner.params.store.push_assistant(Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::Thinking {
                thinking: "old thinking".into(),
                signature: Some("sig1".into()),
            },
            ContentBlock::Text {
                text: "answer 1".into(),
            },
        ],
    });
    runner.params.store.push_user(Message::user("q2"));
    runner.params.store.push_assistant(Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::Thinking {
                thinking: "recent thinking".into(),
                signature: Some("sig2".into()),
            },
            ContentBlock::Text {
                text: "answer 2".into(),
            },
        ],
    });

    let prepared = runner.params.store.prepare_for_llm();

    // First assistant (index 1): thinking should be stripped.
    let first_asst = &prepared[1];
    let has_thinking = first_asst
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::Thinking { .. }));
    assert!(
        !has_thinking,
        "old assistant message should have thinking stripped"
    );

    // Last assistant (index 3): thinking should be preserved.
    let last_asst = &prepared[3];
    let has_thinking = last_asst
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::Thinking { .. }));
    assert!(
        has_thinking,
        "last assistant message should preserve thinking"
    );
}
