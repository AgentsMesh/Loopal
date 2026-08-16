use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{
    ImageOutputPolicy, PermissionLevel, Tool, ToolContext, ToolResult, TypedBridge, TypedTool,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
struct NestedItem {
    label: String,
}

#[derive(Deserialize, JsonSchema)]
struct NestedParams {
    items: Vec<NestedItem>,
}

struct NestedTool;

#[async_trait]
impl TypedTool<NestedParams> for NestedTool {
    fn name(&self) -> &str {
        "Nested"
    }

    fn description(&self) -> &str {
        "test nested schema"
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: NestedParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success(
            input
                .items
                .into_iter()
                .map(|item| item.label)
                .collect::<Vec<_>>()
                .join(","),
        ))
    }
}

#[test]
fn nested_schema_is_fully_inlined_with_safe_defaults() {
    let bridge = TypedBridge::<NestedTool, NestedParams>::new(NestedTool);
    assert_eq!(bridge.dispatch(), loopal_tool_api::ToolDispatch::Pipeline);
    assert_eq!(bridge.image_output_policy(), ImageOutputPolicy::Deny);
    assert_eq!(bridge.precheck(&json!({"items": []})), None);
    let schema = bridge.parameters_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(!schema_str.contains("\"$ref\""));
    assert!(!schema_str.contains("\"definitions\""));
    let items_schema = &schema["properties"]["items"]["items"];
    assert!(items_schema["properties"]["label"].is_object());
}
