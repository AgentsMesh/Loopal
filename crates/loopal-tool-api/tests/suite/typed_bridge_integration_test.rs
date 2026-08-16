use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{
    ImageOutputPolicy, PermissionLevel, Tool, ToolContext, ToolResult, TypedBridge, TypedTool,
};
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

    fn image_output_policy(&self) -> ImageOutputPolicy {
        ImageOutputPolicy::ValidatedInline
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

#[test]
fn typed_bridge_forwards_tool_contract() {
    let bridge = make_bridge();
    assert_eq!(bridge.name(), "Capture");
    assert_eq!(bridge.description(), "test tool");
    assert_eq!(bridge.permission(), PermissionLevel::ReadOnly);
    assert_eq!(bridge.dispatch(), loopal_tool_api::ToolDispatch::Pipeline);
    assert_eq!(
        bridge.image_output_policy(),
        ImageOutputPolicy::ValidatedInline
    );
    assert_eq!(bridge.secret_eligible_params(), &[] as &[&str]);
    assert_eq!(bridge.precheck(&json!({"required_field": "ok"})), None);
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
async fn invalid_typed_input_fails_before_execute() {
    let error = make_bridge()
        .execute(json!({}), &make_ctx())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("required_field"));
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
