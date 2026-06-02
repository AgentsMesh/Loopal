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
// triggered while the agent is idle (no Running/AwaitingInput transitions —
// the agent never leaves WaitingForInput). Pre-fix, Summarize flipped status
// to Running and Done never restored it, leaving the TUI stuck on "Streaming"
// with ESC routing to interrupt and typed input swallowed.
#[test]
fn manual_compact_from_idle_keeps_idle_and_refreshes_ctx() {
    let mut r = idle_reducer();

    r.apply(summarize("259392 tokens before"));
    r.apply(AgentEventPayload::Compacted(CompactionSummary {
        kept: 9,
        removed: 491,
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
}

// Even if the paired TokenUsage emit is dropped, the Compacted event alone
// must leave the reducer self-consistent (idle + refreshed ctx).
#[test]
fn manual_compact_idle_without_token_usage_still_consistent() {
    let mut r = idle_reducer();

    r.apply(summarize("259392 tokens before"));
    r.apply(AgentEventPayload::Compacted(CompactionSummary {
        kept: 9,
        removed: 491,
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
