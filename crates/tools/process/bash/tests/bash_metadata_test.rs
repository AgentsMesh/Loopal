use loopal_tool_api::{PermissionLevel, Tool, TypedBridge};
use loopal_tool_bash::{BashParams, BashTool};

use super::make_store;

fn make_tool() -> TypedBridge<BashTool, BashParams> {
    TypedBridge::new(BashTool::new(make_store()))
}

#[test]
fn test_bash_metadata() {
    let tool = make_tool();
    assert_eq!(tool.name(), "Bash");
    assert!(tool.description().contains("bash"));
    assert_eq!(tool.permission(), PermissionLevel::Dangerous);
    assert_eq!(tool.secret_eligible_params(), &["env"]);

    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["command"].is_object());
    assert!(schema["properties"]["timeout"].is_object());
    assert!(schema["properties"]["process_id"].is_null());
}
