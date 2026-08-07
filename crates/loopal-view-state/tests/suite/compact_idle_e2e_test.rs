use loopal_protocol::{AgentEventPayload, AgentStatus, CompactPhase, CompactionSummary};
use loopal_view_state::ViewStateReducer;

fn idle_reducer() -> ViewStateReducer {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::AwaitingInput);
    assert_eq!(
        r.state().agent.observable.status,
        AgentStatus::WaitingForInput,
        "precondition: reducer must start idle"
    );
    r
}

fn summarize(detail: &str) -> AgentEventPayload {
    AgentEventPayload::CompactProgress {
        phase: CompactPhase::Summarize,
        detail: Some(detail.to_string()),
    }
}

fn done() -> AgentEventPayload {
    AgentEventPayload::CompactProgress {
        phase: CompactPhase::Done,
        detail: None,
    }
}

// Reproduces the exact event sequence the backend emits for a manual /compact
// retry triggered while the agent is idle (no Running/AwaitingInput transitions
// because the agent never leaves WaitingForInput). Retry is nested under the
// compaction operation and must not replace the backend lifecycle.
#[test]
fn manual_compact_from_idle_keeps_idle_and_refreshes_ctx() {
    let mut r = idle_reducer();

    r.apply(summarize("259392 tokens before"));
    r.apply(AgentEventPayload::RetryError {
        message: "HTTP 502. Retrying in 0.1s".into(),
        attempt: 1,
        max_attempts: 3,
    });
    r.apply(AgentEventPayload::RetryCleared);
    r.apply(AgentEventPayload::Compacted(CompactionSummary {
        kept: 9,
        summarized: 491,
        tokens_before: 259_392,
        tokens_after: 6_453,
        strategy: "manual".into(),
        summary_msg_id: None,
        files_rehydrated: 5,
    }));
    r.apply(AgentEventPayload::TokenUsage {
        input_tokens: 6_453,
        output_tokens: 0,
        context_window: 1_000_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        thinking_tokens: 0,
    });
    r.apply(done());

    let state = r.state();
    assert_eq!(
        state.agent.observable.status,
        AgentStatus::WaitingForInput,
        "after /compact the agent must remain idle so ESC/input work normally",
    );
    assert_eq!(
        state.agent.conversation.token_count(),
        6_453,
        "ctx counter must reflect the compacted token total",
    );
    assert_eq!(
        state.agent.conversation.compact_banner, None,
        "Done phase must clear the compacting banner",
    );
    assert!(
        state
            .agent
            .conversation
            .messages
            .iter()
            .any(|message| message.content.contains("5 files rehydrated")),
        "structured Compacted stats must retain the rehydrate result",
    );
}

// Even if the paired TokenUsage emit is dropped, the Compacted event alone
// must leave the reducer self-consistent (idle + refreshed ctx).
#[test]
fn manual_compact_idle_without_token_usage_still_consistent() {
    let mut r = idle_reducer();

    r.apply(summarize("259392 tokens before"));
    r.apply(AgentEventPayload::Compacted(CompactionSummary {
        kept: 9,
        summarized: 491,
        tokens_before: 259_392,
        tokens_after: 6_453,
        strategy: "manual".into(),
        summary_msg_id: None,
        files_rehydrated: 5,
    }));
    r.apply(done());

    let state = r.state();
    assert_eq!(state.agent.observable.status, AgentStatus::WaitingForInput);
    assert_eq!(state.agent.conversation.token_count(), 6_453);
}
