//! Tests for event_handler: apply_event, unified routing, MessageRouted recording.

use loopal_protocol::{AgentEvent, AgentEventPayload, ImageAttachment, UserContent};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

/// Helper: access root agent's conversation field.
macro_rules! conv {
    ($state:expr) => {
        &$state.agents["main"].conversation
    };
}
macro_rules! conv_mut {
    ($state:expr) => {
        &mut $state.agents.get_mut("main").unwrap().conversation
    };
}

#[test]
fn test_apply_event_routes_root_event() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Stream {
            text: "hello".into(),
        }),
    );
    assert_eq!(conv!(state).streaming_text, "hello");
}

#[test]
fn test_apply_event_routes_subagent_event() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("worker", AgentEventPayload::Started),
    );
    assert!(state.agents.contains_key("worker"));
}

#[test]
fn test_apply_event_records_message_routed_to_feed() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::MessageRouted {
            source: "agent-a".into(),
            target: "agent-b".into(),
            content_preview: "test msg".into(),
        }),
    );
    assert_eq!(state.message_feed.len(), 1);
    let entry = state.message_feed.iter().next().unwrap();
    assert_eq!(entry.source, "agent-a");
    assert_eq!(entry.target, "agent-b");
}

#[test]
fn test_apply_event_records_message_routed_to_agent_logs() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("sender", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named("receiver", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::MessageRouted {
            source: "sender".into(),
            target: "receiver".into(),
            content_preview: "hello".into(),
        }),
    );
    assert_eq!(state.agents["sender"].message_log.len(), 1);
    assert_eq!(state.agents["receiver"].message_log.len(), 1);
}

#[test]
fn test_awaiting_input_forwards_inbox() {
    let mut state = make_state();
    state.inbox.push("queued msg".into());
    let forward = apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::AwaitingInput),
    );
    assert_eq!(forward.map(|c| c.text), Some("queued msg".to_string()));
    assert!(!conv!(state).agent_idle); // Immediately busy again
}

#[test]
fn test_awaiting_input_no_inbox_stays_idle() {
    let mut state = make_state();
    let forward = apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::AwaitingInput),
    );
    assert!(forward.is_none());
    assert!(conv!(state).agent_idle);
}

#[test]
fn test_stream_begins_turn() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Stream {
            text: "thinking...".into(),
        }),
    );
    assert_eq!(conv!(state).streaming_text, "thinking...");
}

#[test]
fn test_error_flushes_streaming() {
    let mut state = make_state();
    conv_mut!(state).streaming_text = "partial".to_string();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Error {
            message: "oops".into(),
        }),
    );
    assert!(conv!(state).streaming_text.is_empty());
    assert_eq!(conv!(state).messages.len(), 2); // flushed + error
}

#[test]
fn test_finished_marks_idle() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Finished));
    assert!(conv!(state).agent_idle);
}

#[test]
fn test_token_usage_updates_counters() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            context_window: 200_000,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 80,
            thinking_tokens: 0,
        }),
    );
    assert_eq!(conv!(state).input_tokens, 100);
    assert_eq!(conv!(state).output_tokens, 50);
    assert_eq!(conv!(state).context_window, 200_000);
}

#[test]
fn test_mode_changed_updates_mode() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ModeChanged {
            mode: "plan".into(),
        }),
    );
    // ModeChanged updates the agent's observable.mode, not session-level mode
    assert_eq!(state.agents["main"].observable.mode, "plan");
}

#[test]
fn test_try_forward_inbox_with_images() {
    let mut state = make_state();
    let content = UserContent {
        text: "look at this".to_string(),
        images: vec![ImageAttachment {
            media_type: "image/png".to_string(),
            data: "iVBORw0KGgo=".to_string(),
        }],
        skill_info: None,
    };
    state.inbox.push(content);
    let forward = apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::AwaitingInput),
    );
    let forwarded = forward.expect("should forward inbox content");
    assert_eq!(forwarded.text, "look at this");
    assert_eq!(forwarded.images.len(), 1);
    assert_eq!(forwarded.images[0].media_type, "image/png");
    let display = conv!(state).messages.last().unwrap();
    assert_eq!(display.role, "user");
    assert!(display.content.contains("[+1 image(s)]"));
    assert_eq!(display.image_count, 1);
    assert!(!conv!(state).agent_idle);
}
