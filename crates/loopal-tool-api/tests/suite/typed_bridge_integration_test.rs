#![allow(dead_code)]

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult, TypedBridge, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
struct TestParams {
    required_field: String,
    #[serde(default)]
    optional_str: Option<String>,
    #[serde(default)]
    optional_num: Option<u64>,
}

struct CaptureTool;

#[async_trait]
impl TypedTool<TestParams> for CaptureTool {
    fn name(&self) -> &str {
        "Capture"
    }
    fn description(&self) -> &str {
        "test tool"
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }
    async fn execute(
        &self,
        input: TestParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let has_str = input.optional_str.is_some();
        let has_num = input.optional_num.is_some();
        Ok(ToolResult::success(format!(
            "required={} optional_str={} optional_num={}",
            input.required_field, has_str, has_num
        )))
    }
}

fn make_bridge() -> TypedBridge<CaptureTool, TestParams> {
    TypedBridge::new(CaptureTool)
}

fn make_ctx() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "t")
}

#[tokio::test]
async fn empty_string_optional_becomes_none() {
    let bridge = make_bridge();
    let input = json!({"required_field": "hello", "optional_str": ""});
    let result = bridge.execute(input, &make_ctx()).await.unwrap();
    assert_eq!(
        result.content,
        "required=hello optional_str=false optional_num=false"
    );
}

#[tokio::test]
async fn empty_string_on_required_field_is_preserved() {
    let bridge = make_bridge();
    let input = json!({"required_field": "", "optional_str": "val"});
    let result = bridge.execute(input, &make_ctx()).await.unwrap();
    assert_eq!(
        result.content,
        "required= optional_str=true optional_num=false"
    );
}

#[tokio::test]
async fn absent_optional_fields_are_none() {
    let bridge = make_bridge();
    let input = json!({"required_field": "x"});
    let result = bridge.execute(input, &make_ctx()).await.unwrap();
    assert_eq!(
        result.content,
        "required=x optional_str=false optional_num=false"
    );
}

#[tokio::test]
async fn non_empty_optional_string_is_some() {
    let bridge = make_bridge();
    let input = json!({"required_field": "x", "optional_str": "present"});
    let result = bridge.execute(input, &make_ctx()).await.unwrap();
    assert_eq!(
        result.content,
        "required=x optional_str=true optional_num=false"
    );
}

#[tokio::test]
async fn schema_has_no_dollar_schema_or_title() {
    let bridge = make_bridge();
    let schema = bridge.parameters_schema();
    assert!(schema.get("$schema").is_none());
    assert!(schema.get("title").is_none());
}

#[tokio::test]
async fn schema_has_no_ref_or_definitions() {
    let bridge = make_bridge();
    let schema_str = serde_json::to_string(&bridge.parameters_schema()).unwrap();
    assert!(!schema_str.contains("\"$ref\""));
    assert!(!schema_str.contains("\"definitions\""));
    assert!(!schema_str.contains("\"$defs\""));
}

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
        "test"
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }
    async fn execute(
        &self,
        _input: NestedParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success("ok"))
    }
}

#[test]
fn nested_schema_is_fully_inlined() {
    let bridge = TypedBridge::<NestedTool, NestedParams>::new(NestedTool);
    let schema = bridge.parameters_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(!schema_str.contains("\"$ref\""));
    assert!(!schema_str.contains("\"definitions\""));
    let items_schema = &schema["properties"]["items"]["items"];
    assert!(items_schema["properties"]["label"].is_object());
}
