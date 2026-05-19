use loopal_context::ContextBudget;
use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::StreamChunk;
use loopal_test_support::{HarnessBuilder, chunks};

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

fn budget_window(window: u32) -> ContextBudget {
    ContextBudget {
        context_window: window,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: window / 10,
        safety_margin: window / 20,
        message_budget: window - window / 10 - window / 20,
        max_output_tokens: window / 10,
    }
}

/// Provider-reported `input_tokens` propagates into the store and
/// `effective_tokens()` reflects the API's reading even when the local
/// tiktoken estimate is much smaller. This is the core defense that the
/// original 920K → 502 incident was missing.
#[tokio::test]
async fn usage_chunk_drives_effective_tokens_above_estimate() {
    let high_input_tokens: u32 = 800_000;
    let calls = vec![vec![
        Ok(StreamChunk::Text { text: "ack".into() }),
        chunks::usage(high_input_tokens, 50),
        chunks::done(),
    ]];

    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("trigger one turn")])
        .build()
        .await;

    h.runner
        .params
        .store
        .update_budget(budget_window(1_000_000));

    let estimate_before = h.runner.params.store.current_tokens();
    assert!(
        estimate_before < high_input_tokens,
        "local estimate ({estimate_before}) must be the underestimate in this test",
    );

    let _ = h.runner.run().await;

    let effective_after = h.runner.params.store.effective_tokens();
    assert!(
        effective_after >= high_input_tokens,
        "effective_tokens ({effective_after}) must rise to actual input ({high_input_tokens})",
    );

    let _ = drain_events(&mut h.event_rx).await;
}

/// With effective_tokens lifted by the API feedback, the next compaction
/// check must trip the 80% threshold — even though tiktoken alone would
/// still report a tiny conversation. Closes the regression loop on the
/// "estimator-too-low never triggers auto-compact" failure mode.
#[tokio::test]
async fn api_token_feedback_triggers_next_auto_compact() {
    let context_window: u32 = 1_000_000;
    let actual_input: u32 = context_window * 85 / 100; // above 80%

    let calls = vec![vec![
        Ok(StreamChunk::Text {
            text: "first".into(),
        }),
        chunks::usage(actual_input, 10),
        chunks::done(),
    ]];

    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("first turn")])
        .build()
        .await;

    h.runner
        .params
        .store
        .update_budget(budget_window(context_window));

    assert!(
        !h.runner.params.store.needs_summarization(),
        "tiny conversation should not yet need compaction",
    );

    let _ = h.runner.run().await;

    assert!(
        h.runner.params.store.needs_summarization(),
        "after Usage chunk reports {actual_input} tokens (85% of {context_window}), \
         needs_summarization must trip on the next check",
    );
}
