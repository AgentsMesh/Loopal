use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

struct OversizedTool;

#[async_trait]
impl Tool for OversizedTool {
    fn name(&self) -> &str {
        "FinalSinkOversized"
    }

    fn description(&self) -> &str {
        "test final sink bound"
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object", "additionalProperties": false})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success(
            "x".repeat(loopal_tool_api::DEFAULT_MAX_OUTPUT_BYTES + 1),
        ))
    }
}

fn matching_overflow_files() -> std::collections::BTreeSet<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("loopal").join("overflow");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Default::default();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("FinalSinkOversized_"))
        })
        .collect()
}

#[tokio::test]
async fn final_byte_limit_rejects_before_overflow_file_write() {
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(OversizedTool));
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "final-sink-test",
    );
    let ctx = ToolContext::new(backend, "final-sink-test");
    let action = prepare_tool_action(&kernel, "id", "FinalSinkOversized", json!({}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    let before = matching_overflow_files();
    let error = execute_tool(&kernel, action, &ctx, &AgentMode::Act)
        .await
        .unwrap_err();
    let after = matching_overflow_files();
    assert!(error.to_string().contains("final byte limit"));
    assert_eq!(after, before);
}
