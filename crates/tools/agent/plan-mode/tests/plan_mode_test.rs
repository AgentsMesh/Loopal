use loopal_tool_api::{PermissionLevel, Tool};
use loopal_tool_plan_mode::{EnterPlanModeTool, ExitPlanModeTool};

#[test]
fn enter_plan_mode_name() {
    assert_eq!(EnterPlanModeTool.name(), "EnterPlanMode");
}

#[test]
fn enter_plan_mode_description() {
    let desc = EnterPlanModeTool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("plan mode"), "should mention plan mode");
    assert!(
        desc.contains("When to use") || desc.contains("non-trivial"),
        "should provide usage guidance"
    );
}

#[test]
fn enter_plan_mode_permission() {
    assert_eq!(EnterPlanModeTool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn exit_plan_mode_name() {
    assert_eq!(ExitPlanModeTool.name(), "ExitPlanMode");
}

#[test]
fn exit_plan_mode_description() {
    let desc = ExitPlanModeTool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("plan"), "should mention plan");
    assert!(
        desc.contains("plan file") || desc.contains("approval"),
        "should explain plan file or approval mechanism"
    );
}

#[test]
fn exit_plan_mode_permission() {
    assert_eq!(ExitPlanModeTool.permission(), PermissionLevel::ReadOnly);
}
