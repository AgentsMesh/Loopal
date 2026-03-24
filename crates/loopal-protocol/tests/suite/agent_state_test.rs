use loopal_protocol::{AgentStatus, ObservableAgentState};

#[test]
fn test_agent_status_default_is_starting() {
    assert_eq!(AgentStatus::default(), AgentStatus::Starting);
}

#[test]
fn test_agent_status_all_variants_debug() {
    let variants = [
        AgentStatus::Starting,
        AgentStatus::Running,
        AgentStatus::WaitingForInput,
        AgentStatus::Finished,
        AgentStatus::Error,
    ];
    for v in &variants {
        let debug = format!("{v:?}");
        assert!(!debug.is_empty());
    }
}

#[test]
fn test_agent_status_serde_roundtrip() {
    for status in [
        AgentStatus::Starting,
        AgentStatus::Running,
        AgentStatus::WaitingForInput,
        AgentStatus::Finished,
        AgentStatus::Error,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let restored: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, status);
    }
}

#[test]
fn test_observable_agent_state_default() {
    let state = ObservableAgentState::default();
    assert_eq!(state.status, AgentStatus::Starting);
    assert_eq!(state.tool_count, 0);
    assert!(state.last_tool.is_none());
    assert_eq!(state.turn_count, 0);
    assert_eq!(state.input_tokens, 0);
    assert_eq!(state.output_tokens, 0);
    assert!(state.model.is_empty());
    assert_eq!(state.mode, "act");
    assert_eq!(state.tools_in_flight, 0);
}

#[test]
fn test_observable_agent_state_serde_roundtrip() {
    let state = ObservableAgentState {
        status: AgentStatus::Running,
        tool_count: 5,
        last_tool: Some("Read".to_string()),
        turn_count: 3,
        input_tokens: 1000,
        output_tokens: 500,
        model: "claude-sonnet".to_string(),
        mode: "plan".to_string(),
        tools_in_flight: 0,
    };
    let json = serde_json::to_string(&state).unwrap();
    let restored: ObservableAgentState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.status, AgentStatus::Running);
    assert_eq!(restored.tool_count, 5);
    assert_eq!(restored.last_tool, Some("Read".to_string()));
    assert_eq!(restored.turn_count, 3);
}
