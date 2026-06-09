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
        AgentStatus::Suspended,
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
        AgentStatus::Suspended,
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
    assert_eq!(state.turn_count, 0);
    assert_eq!(state.input_tokens, 0);
    assert_eq!(state.output_tokens, 0);
    assert!(state.model.is_empty());
    assert_eq!(state.thinking_config, "auto");
    assert_eq!(state.mode, "act");
}

#[test]
fn test_observable_agent_state_serde_roundtrip() {
    let state = ObservableAgentState {
        status: AgentStatus::Running,
        turn_count: 3,
        input_tokens: 1000,
        output_tokens: 500,
        model: "claude-sonnet".to_string(),
        thinking_config: "effort".to_string(),
        mode: "plan".to_string(),
        permission_mode: "bypass".to_string(),
        decision_mode: "manual".to_string(),
        sandbox_policy: "default_write".to_string(),
    };
    let json = serde_json::to_string(&state).unwrap();
    let restored: ObservableAgentState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.status, AgentStatus::Running);
    assert_eq!(restored.turn_count, 3);
    assert_eq!(restored.thinking_config, "effort");
    assert_eq!(restored.permission_mode, "bypass");
    assert_eq!(restored.decision_mode, "manual");
    assert_eq!(restored.sandbox_policy, "default_write");
}

#[test]
fn test_observable_thinking_config_defaults_when_absent() {
    let legacy_json = r#"{
        "status": "Running",
        "turn_count": 0,
        "input_tokens": 0,
        "output_tokens": 0,
        "model": "claude-x",
        "mode": "act"
    }"#;
    let restored: ObservableAgentState = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(restored.thinking_config, "auto");
}
