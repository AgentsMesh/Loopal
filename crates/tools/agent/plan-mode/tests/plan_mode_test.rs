use loopal_tool_api::{PermissionLevel, Tool, TypedBridge};
use loopal_tool_plan_mode::{
    EnterPlanModeParams, EnterPlanModeTool, ExitPlanModeParams, ExitPlanModeTool,
};

fn make_enter_tool() -> impl Tool {
    TypedBridge::<EnterPlanModeTool, EnterPlanModeParams>::new(EnterPlanModeTool)
}

fn make_exit_tool() -> impl Tool {
    TypedBridge::<ExitPlanModeTool, ExitPlanModeParams>::new(ExitPlanModeTool)
}

#[test]
fn enter_plan_mode_name() {
    assert_eq!(make_enter_tool().name(), "EnterPlanMode");
}

#[test]
fn enter_plan_mode_description() {
    let tool = make_enter_tool();
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("plan mode"), "should mention plan mode");
    assert!(
        desc.contains("When to use") || desc.contains("non-trivial"),
        "should provide usage guidance"
    );
}

#[test]
fn enter_plan_mode_permission() {
    assert_eq!(make_enter_tool().permission(), PermissionLevel::ReadOnly);
}

#[test]
fn exit_plan_mode_name() {
    assert_eq!(make_exit_tool().name(), "ExitPlanMode");
}

#[test]
fn exit_plan_mode_description() {
    let tool = make_exit_tool();
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("plan"), "should mention plan");
    assert!(
        desc.contains("plan file") || desc.contains("approval"),
        "should explain plan file or approval mechanism"
    );
}

#[test]
fn exit_plan_mode_permission() {
    assert_eq!(make_exit_tool().permission(), PermissionLevel::ReadOnly);
}
