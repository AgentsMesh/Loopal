use loopal_tool_api::{PermissionLevel, Tool, TypedBridge};
use loopal_tool_ask_user::{AskUserParams, AskUserTool};
use serde_json::json;

fn make_tool() -> impl Tool {
    TypedBridge::<AskUserTool, AskUserParams>::new(AskUserTool)
}

#[test]
fn test_ask_user_name() {
    assert_eq!(make_tool().name(), "AskUser");
}

#[test]
fn test_ask_user_permission() {
    assert_eq!(make_tool().permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_ask_user_description() {
    let tool = make_tool();
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("question"));
}

#[test]
fn test_ask_user_parameters_schema() {
    let schema = make_tool().parameters_schema();
    assert_eq!(schema["type"], "object");

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("questions")));

    let questions = &schema["properties"]["questions"];
    assert_eq!(questions["type"], "array");
}
