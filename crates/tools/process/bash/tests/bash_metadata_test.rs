use loopal_tool_api::{PermissionLevel, Tool};
use loopal_tool_bash::BashTool;

use super::make_store;

#[test]
fn test_bash_metadata() {
    let tool = BashTool::new(make_store());
    assert_eq!(tool.name(), "Bash");
    assert!(tool.description().contains("bash"));
    assert_eq!(tool.permission(), PermissionLevel::Dangerous);

    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["command"].is_object());
    assert!(schema["properties"]["process_id"].is_object());
    assert!(schema["properties"]["timeout"].is_object());
}
