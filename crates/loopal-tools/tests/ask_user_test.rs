use loopal_tool_api::{PermissionLevel, Tool};
use loopal_tools::builtin::ask_user::AskUserTool;
use serde_json::json;

#[test]
fn test_ask_user_name() {
    let tool = AskUserTool;
    assert_eq!(tool.name(), "AskUser");
}

#[test]
fn test_ask_user_permission() {
    let tool = AskUserTool;
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_ask_user_description() {
    let tool = AskUserTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("question"));
}

#[test]
fn test_ask_user_parameters_schema() {
    let tool = AskUserTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("questions")));

    let questions = &schema["properties"]["questions"];
    assert_eq!(questions["type"], "array");

    let item_props = &questions["items"]["properties"];
    assert!(item_props["question"].is_object());
    assert!(item_props["options"].is_object());
    assert!(item_props["multiSelect"].is_object());

    let item_required = questions["items"]["required"].as_array().unwrap();
    assert!(item_required.contains(&json!("question")));
    assert!(item_required.contains(&json!("options")));
}

#[test]
fn test_ask_user_options_schema() {
    let tool = AskUserTool;
    let schema = tool.parameters_schema();

    let option_props =
        &schema["properties"]["questions"]["items"]["properties"]["options"]["items"]["properties"];
    assert!(option_props["label"].is_object());
    assert!(option_props["description"].is_object());
}
