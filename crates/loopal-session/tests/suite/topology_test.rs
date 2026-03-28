//! Tests for topology registration via SubAgentSpawned events.

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::event_handler::apply_event;

use super::agent_lifecycle_test::make_state;

#[test]
fn spawned_agent_has_parent_set() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SubAgentSpawned {
            name: "worker".into(),
            agent_id: "id-1".into(),
            parent: Some("root-agent".into()),
            model: Some("claude-haiku".into()),
        }),
    );
    assert!(state.agents.contains_key("worker"));
    assert_eq!(state.agents["worker"].parent.as_deref(), Some("root-agent"));
    assert_eq!(state.agents["worker"].observable.model, "claude-haiku");
}

#[test]
fn spawned_agent_registered_as_child_of_parent() {
    let mut state = make_state();

    // Parent must exist first (created by a Started event)
    apply_event(
        &mut state,
        AgentEvent::named("researcher", AgentEventPayload::Started),
    );

    // Spawn child under researcher
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SubAgentSpawned {
            name: "coder".into(),
            agent_id: "id-2".into(),
            parent: Some("researcher".into()),
            model: None,
        }),
    );

    assert!(
        state.agents["researcher"]
            .children
            .contains(&"coder".into())
    );
    assert_eq!(state.agents["coder"].parent.as_deref(), Some("researcher"));
}

#[test]
fn spawned_agent_without_parent_is_root_level() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SubAgentSpawned {
            name: "solo".into(),
            agent_id: "id-3".into(),
            parent: None,
            model: None,
        }),
    );
    assert!(state.agents.contains_key("solo"));
    assert!(state.agents["solo"].parent.is_none());
}

#[test]
fn subsequent_events_preserve_topology() {
    let mut state = make_state();

    // Spawn with parent
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SubAgentSpawned {
            name: "worker".into(),
            agent_id: "id-4".into(),
            parent: Some("boss".into()),
            model: Some("claude-sonnet-4".into()),
        }),
    );

    // Subsequent events should not overwrite parent/children
    apply_event(
        &mut state,
        AgentEvent::named("worker", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "worker",
            AgentEventPayload::ToolCall {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({}),
            },
        ),
    );

    assert_eq!(state.agents["worker"].parent.as_deref(), Some("boss"));
    assert_eq!(state.agents["worker"].observable.model, "claude-sonnet-4");
}

#[test]
fn duplicate_spawn_does_not_add_child_twice() {
    let mut state = make_state();

    apply_event(
        &mut state,
        AgentEvent::named("parent", AgentEventPayload::Started),
    );

    for _ in 0..2 {
        apply_event(
            &mut state,
            AgentEvent::root(AgentEventPayload::SubAgentSpawned {
                name: "child".into(),
                agent_id: "id-5".into(),
                parent: Some("parent".into()),
                model: None,
            }),
        );
    }

    assert_eq!(
        state.agents["parent"]
            .children
            .iter()
            .filter(|c| *c == "child")
            .count(),
        1,
        "child should appear exactly once in parent's children list"
    );
}
