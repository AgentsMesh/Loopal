use loopagent_types::command::AgentMode;

#[test]
fn test_agent_mode_equality() {
    assert_eq!(AgentMode::Act, AgentMode::Act);
    assert_eq!(AgentMode::Plan, AgentMode::Plan);
    assert_ne!(AgentMode::Act, AgentMode::Plan);
    assert_ne!(AgentMode::Plan, AgentMode::Act);
}

#[test]
fn test_agent_mode_clone() {
    let mode = AgentMode::Plan;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_agent_mode_debug() {
    let act = format!("{:?}", AgentMode::Act);
    let plan = format!("{:?}", AgentMode::Plan);
    assert_eq!(act, "Act");
    assert_eq!(plan, "Plan");
}
