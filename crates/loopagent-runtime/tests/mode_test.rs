use loopagent_runtime::AgentMode;
use loopagent_types::command::AgentMode as TypesAgentMode;
use loopagent_types::permission::PermissionMode;

#[test]
fn test_act_mode_empty_suffix() {
    assert_eq!(AgentMode::Act.system_prompt_suffix(), "");
}

#[test]
fn test_plan_mode_has_suffix() {
    let suffix = AgentMode::Plan.system_prompt_suffix();
    assert!(suffix.contains("PLAN mode"));
    assert!(suffix.contains("cannot make any changes"));
}

#[test]
fn test_act_permission_mode() {
    assert_eq!(AgentMode::Act.permission_mode(), PermissionMode::Default);
}

#[test]
fn test_plan_permission_mode() {
    assert_eq!(AgentMode::Plan.permission_mode(), PermissionMode::Plan);
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
