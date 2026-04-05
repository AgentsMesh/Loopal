//! Multi-agent tracking and event routing tests for SessionState.
//! Covers independent agent tracking and root vs. named event routing.

use loopal_protocol::{AgentEvent, AgentEventPayload, AgentStatus};
use loopal_session::event_handler::apply_event;

use super::agent_lifecycle_test::{apply_sequence, make_state};

// -- Multi-agent tracking -------------------------------------------------

/// Two sub-agents tracked independently in SessionState.
#[test]
fn multiple_agents_tracked_independently() {
    let mut state = make_state();

    apply_sequence(
        &mut state,
        "researcher",
        vec![
            AgentEventPayload::Started,
            AgentEventPayload::Stream {
                text: "researching...".into(),
            },
        ],
    );
    apply_sequence(
        &mut state,
        "coder",
        vec![
            AgentEventPayload::Started,
            AgentEventPayload::ToolCall {
                id: "tc-w".into(),
                name: "Write".into(),
                input: serde_json::json!({}),
            },
        ],
    );

    // "main" + "researcher" + "coder" = 3 agents
    assert_eq!(state.agents.len(), 3);
    assert_eq!(
        state.agents["researcher"].observable.status,
        AgentStatus::Running
    );
    assert_eq!(state.agents["researcher"].observable.tool_count, 0);
    assert_eq!(
        state.agents["coder"].observable.status,
        AgentStatus::Running
    );
    assert_eq!(state.agents["coder"].observable.tool_count, 1);

    // Researcher finishes, coder continues
    apply_event(
        &mut state,
        AgentEvent::named("researcher", AgentEventPayload::Finished),
    );
    assert_eq!(
        state.agents["researcher"].observable.status,
        AgentStatus::Finished
    );
    assert_eq!(
        state.agents["coder"].observable.status,
        AgentStatus::Running
    );
}

// -- Event routing --------------------------------------------------------

/// Root events (agent_name=None) route to "main" agent in the agents map.
#[test]
fn root_events_route_to_main_agent() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Stream {
            text: "hello".into(),
        }),
    );
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Finished));
    // "main" exists (created at init), root events route there
    assert!(state.agents.contains_key("main"));
    assert!(state.agents["main"].is_idle());
}

/// SubAgentSpawned event creates an AgentViewState entry with topology info.
#[test]
fn sub_agent_spawned_registers_topology() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SubAgentSpawned {
            name: "worker".into(),
            agent_id: "test-id".into(),
            parent: None,
            model: Some("claude-sonnet-4".into()),
            session_id: None,
        }),
    );
    assert!(
        state.agents.contains_key("worker"),
        "SubAgentSpawned should create agent entry"
    );
    assert_eq!(state.agents["worker"].observable.model, "claude-sonnet-4");
}

/// First real event from sub-agent creates the AgentViewState entry.
#[test]
fn first_event_creates_agent_view_state() {
    let mut state = make_state();
    assert!(!state.agents.contains_key("w1"));

    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    assert!(state.agents.contains_key("w1"));
    assert!(state.agents["w1"].started_at.is_some());
}
