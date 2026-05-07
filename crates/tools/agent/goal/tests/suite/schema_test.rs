use loopal_tool_api::{PermissionLevel, Tool};
use loopal_tool_goal::{CreateGoalTool, GetGoalTool, UpdateGoalTool};

#[test]
fn names_are_snake_case() {
    assert_eq!(GetGoalTool.name(), "get_goal");
    assert_eq!(CreateGoalTool.name(), "create_goal");
    assert_eq!(UpdateGoalTool.name(), "update_goal");
}

#[test]
fn permission_levels_match_intent() {
    assert_eq!(GetGoalTool.permission(), PermissionLevel::ReadOnly);
    assert_eq!(CreateGoalTool.permission(), PermissionLevel::Supervised);
    assert_eq!(UpdateGoalTool.permission(), PermissionLevel::Supervised);
}

#[test]
fn update_goal_status_enum_only_exposes_complete() {
    let schema = UpdateGoalTool.parameters_schema();
    let status_enum = schema["properties"]["status"]["enum"]
        .as_array()
        .expect("status.enum must be an array");
    let values: Vec<&str> = status_enum.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(values, vec!["complete"]);
}

#[test]
fn create_goal_marks_objective_required() {
    let schema = CreateGoalTool.parameters_schema();
    let required = schema["required"]
        .as_array()
        .expect("required must be array");
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "objective");
}

#[test]
fn create_goal_token_budget_has_minimum() {
    let schema = CreateGoalTool.parameters_schema();
    assert_eq!(schema["properties"]["token_budget"]["minimum"], 1);
}

#[test]
fn create_goal_description_warns_against_inferring() {
    assert!(
        CreateGoalTool
            .description()
            .to_lowercase()
            .contains("explicit"),
        "description must signal explicit-request requirement"
    );
}

#[test]
fn update_goal_description_warns_against_premature_complete() {
    let desc = UpdateGoalTool.description().to_lowercase();
    assert!(desc.contains("achieved"), "must say 'achieved'");
    assert!(
        desc.contains("budget") && desc.contains("not"),
        "must warn against marking complete due to budget"
    );
}

#[test]
fn get_goal_takes_no_arguments() {
    let schema = GetGoalTool.parameters_schema();
    let props = schema["properties"]
        .as_object()
        .expect("properties must be object");
    assert!(props.is_empty(), "get_goal must take no arguments");
}
