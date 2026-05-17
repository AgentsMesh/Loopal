use loopal_tool_api::{PermissionLevel, Tool};

use super::support::{make_create_goal_tool, make_get_goal_tool, make_update_goal_tool};

#[test]
fn names_are_snake_case() {
    assert_eq!(make_get_goal_tool().name(), "get_goal");
    assert_eq!(make_create_goal_tool().name(), "create_goal");
    assert_eq!(make_update_goal_tool().name(), "update_goal");
}

#[test]
fn permission_levels_match_intent() {
    assert_eq!(make_get_goal_tool().permission(), PermissionLevel::ReadOnly);
    assert_eq!(make_create_goal_tool().permission(), PermissionLevel::Write);
    assert_eq!(make_update_goal_tool().permission(), PermissionLevel::Write);
}

#[test]
fn create_goal_marks_objective_required() {
    let schema = make_create_goal_tool().parameters_schema();
    let required = schema["required"]
        .as_array()
        .expect("required must be array");
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "objective");
}

#[test]
fn create_goal_description_warns_against_inferring() {
    assert!(
        make_create_goal_tool()
            .description()
            .to_lowercase()
            .contains("explicit"),
        "description must signal explicit-request requirement"
    );
}

#[test]
fn update_goal_description_warns_against_premature_complete() {
    let desc = make_update_goal_tool().description().to_lowercase();
    assert!(desc.contains("achieved"), "must say 'achieved'");
    assert!(
        desc.contains("stopping work") && desc.contains("not"),
        "must warn against marking complete merely because work is stopping"
    );
}
