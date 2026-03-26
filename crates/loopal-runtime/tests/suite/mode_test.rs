use loopal_protocol::AgentMode as TypesAgentMode;
use loopal_runtime::AgentMode;

#[test]
fn test_act_mode_empty_suffix() {
    assert_eq!(AgentMode::Act.system_prompt_suffix(), "");
}

#[test]
fn test_plan_mode_has_suffix() {
    let suffix = AgentMode::Plan.system_prompt_suffix();
    assert!(suffix.contains("PLAN mode"));
    assert!(suffix.contains("Explore the codebase"));
}

#[test]
fn test_from_types_act() {
    let mode: AgentMode = TypesAgentMode::Act.into();
    assert_eq!(mode, AgentMode::Act);
}

#[test]
fn test_from_types_plan() {
    let mode: AgentMode = TypesAgentMode::Plan.into();
    assert_eq!(mode, AgentMode::Plan);
}
