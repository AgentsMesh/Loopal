use async_trait::async_trait;
use loopal_config::{HookConfig, HookEvent, Settings};
use loopal_kernel::Kernel;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use super::*;

struct ValidatedTool;

#[async_trait]
impl Tool for ValidatedTool {
    fn name(&self) -> &str {
        "Validated"
    }

    fn description(&self) -> &str {
        "test tool"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {"value": {"type": "string"}},
            "required": ["value"],
            "additionalProperties": false
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn precheck(&self, input: &Value) -> Option<String> {
        (input["value"] == "blocked").then(|| "blocked value".into())
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> loopal_error::Result<ToolResult> {
        panic!("preparation test must not execute the tool")
    }
}

fn kernel(marker: &std::path::Path) -> Kernel {
    let command = format!(
        "printf ran > '{}'; printf '%s' '{{\"updated_input\":{{\"value\":\"repaired\"}}}}'",
        marker.display()
    );
    let settings = Settings {
        hooks: vec![HookConfig {
            event: HookEvent::PreToolUse,
            command,
            tool_filter: Some(vec!["Validated".into()]),
            timeout_ms: 5_000,
            hook_type: Default::default(),
            url: None,
            headers: Default::default(),
            prompt: None,
            model: None,
            condition: None,
            id: None,
        }],
        ..Default::default()
    };
    let kernel = Kernel::new(settings).unwrap();
    kernel.register_tool(Box::new(ValidatedTool));
    kernel
}

fn rewrite_hook(value: &str) -> HookConfig {
    HookConfig {
        event: HookEvent::PreToolUse,
        command: format!("printf '%s' '{{\"updated_input\":{{\"value\":\"{value}\"}}}}'"),
        tool_filter: Some(vec!["Validated".into()]),
        timeout_ms: 5_000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }
}

#[tokio::test]
async fn invalid_original_inputs_never_reach_pre_hooks() {
    let marker = std::env::temp_dir().join(format!(
        "loopal-invalid-original-hook-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&marker);
    let kernel = kernel(&marker);

    for input in [
        json!({}),
        json!({"value": "<secret_ref:token>"}),
        json!({"value": "blocked"}),
    ] {
        let result = prepare_tool_action(&kernel, "id", "Validated", input)
            .await
            .unwrap();
        assert!(matches!(result, ToolPreparation::Denied(_)));
        assert!(!marker.exists(), "pre-hook ran for invalid input");
    }
}

#[tokio::test]
async fn later_rewrite_wins_when_multiple_pre_hooks_update_input() {
    let settings = Settings {
        hooks: vec![rewrite_hook("first"), rewrite_hook("second")],
        ..Default::default()
    };
    let kernel = Kernel::new(settings).unwrap();
    kernel.register_tool(Box::new(ValidatedTool));

    let prepared = prepare_tool_action(&kernel, "id", "Validated", json!({"value": "original"}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();

    assert!(prepared.was_rewritten());
    assert_eq!(prepared.placeholder_input(), &json!({"value": "second"}));
}
