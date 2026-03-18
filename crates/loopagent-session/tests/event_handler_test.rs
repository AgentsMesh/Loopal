//! Tests for event_handler: apply_event, apply_root_event, MessageRouted recording.

use loopagent_session::event_handler::apply_event;
use loopagent_session::state::SessionState;
use loopagent_types::event::{AgentEvent, AgentEventPayload};

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

#[test]
fn test_apply_event_routes_root_event() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Stream {
        text: "hello".into(),
    }));
    assert_eq!(state.streaming_text, "hello");
}

#[test]
fn test_apply_event_routes_subagent_event() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::named("worker", AgentEventPayload::Started));
    assert!(state.agents.contains_key("worker"));
}

#[test]
fn test_apply_event_records_message_routed_to_feed() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::MessageRouted {
        source: "agent-a".into(),
        target: "agent-b".into(),
        content_preview: "test msg".into(),
    }));
    assert_eq!(state.message_feed.len(), 1);
    let entry = state.message_feed.iter().next().unwrap();
    assert_eq!(entry.source, "agent-a");
    assert_eq!(entry.target, "agent-b");
}

#[test]
fn test_apply_event_records_message_routed_to_agent_logs() {
    let mut state = make_state();
    // Create agent entries first
    apply_event(&mut state, AgentEvent::named("sender", AgentEventPayload::Started));
    apply_event(&mut state, AgentEvent::named("receiver", AgentEventPayload::Started));

    apply_event(&mut state, AgentEvent::root(AgentEventPayload::MessageRouted {
        source: "sender".into(),
        target: "receiver".into(),
        content_preview: "hello".into(),
    }));

    assert_eq!(state.agents["sender"].message_log.len(), 1);
    assert_eq!(state.agents["receiver"].message_log.len(), 1);
}

#[test]
fn test_awaiting_input_forwards_inbox() {
    let mut state = make_state();
    state.inbox.push("queued msg".to_string());
    let forward = apply_event(&mut state, AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert_eq!(forward, Some("queued msg".to_string()));
    assert!(!state.agent_idle); // Immediately busy again
}

#[test]
fn test_awaiting_input_no_inbox_stays_idle() {
    let mut state = make_state();
    let forward = apply_event(&mut state, AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert!(forward.is_none());
    assert!(state.agent_idle);
}

#[test]
fn test_stream_begins_turn() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Stream {
        text: "thinking...".into(),
    }));
    // turn_start should be set (indirectly: turn_elapsed > 0 eventually)
    assert_eq!(state.streaming_text, "thinking...");
}

#[test]
fn test_error_flushes_streaming() {
    let mut state = make_state();
    state.streaming_text = "partial".to_string();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Error {
        message: "oops".into(),
    }));
    assert!(state.streaming_text.is_empty());
    assert_eq!(state.messages.len(), 2); // flushed + error
}

#[test]
fn test_finished_marks_idle() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Finished));
    assert!(state.agent_idle);
}

#[test]
fn test_token_usage_updates_counters() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 10,
        cache_read_input_tokens: 80,
    }));
    assert_eq!(state.input_tokens, 100);
    assert_eq!(state.output_tokens, 50);
    assert_eq!(state.context_window, 200_000);
}

#[test]
fn test_mode_changed_updates_mode() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::ModeChanged {
        mode: "plan".into(),
    }));
    assert_eq!(state.mode, "plan");
}
