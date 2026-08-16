use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_config::{HookConfig, HookEvent, Settings};
use loopal_kernel::Kernel;
use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

struct BoundaryTool {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for BoundaryTool {
    fn name(&self) -> &str {
        "Boundary"
    }

    fn description(&self) -> &str {
        "Effect-boundary fixture"
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

    async fn execute(&self, _: Value, _: &ToolContext) -> loopal_error::Result<ToolResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolResult::success("executed"))
    }
}

fn hook(value: &str) -> HookConfig {
    let output = json!({"updated_input": {"value": value}}).to_string();
    HookConfig {
        event: HookEvent::PreToolUse,
        command: format!("printf '%s' '{output}'"),
        tool_filter: Some(vec!["Boundary".into()]),
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

fn kernel(rewrite: Option<&str>) -> (Kernel, Arc<AtomicUsize>) {
    let settings = Settings {
        hooks: rewrite.into_iter().map(hook).collect(),
        ..Default::default()
    };
    let kernel = Kernel::new(settings).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    kernel.register_tool(Box::new(BoundaryTool {
        calls: calls.clone(),
    }));
    (kernel, calls)
}

fn context() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "effect-boundary",
    );
    ToolContext::new(backend, "effect-boundary")
        .with_protected_effect_audit(Arc::new(loopal_tool_api::NoopProtectedEffectAudit))
}

async fn rewritten_error(value: &str) -> (String, usize) {
    let (kernel, calls) = kernel(Some(value));
    let action = prepare_tool_action(&kernel, "id", "Boundary", json!({"value": "safe"}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    let error = execute_tool(&kernel, action, &context(), &AgentMode::Act)
        .await
        .unwrap_err()
        .to_string();
    (error, calls.load(Ordering::SeqCst))
}

#[tokio::test]
async fn rewritten_wire_ref_and_precheck_fail_before_execution() {
    let (wire_error, wire_calls) = rewritten_error("<secret_ref:token>").await;
    assert!(wire_error.contains("wire-ref validation failed"));
    assert_eq!(wire_calls, 0);

    let (precheck_error, precheck_calls) = rewritten_error("blocked").await;
    assert!(precheck_error.contains("tool precheck failed"));
    assert_eq!(precheck_calls, 0);
}

#[tokio::test]
async fn same_name_tool_replacement_invalidates_prepared_action() {
    let (kernel, old_calls) = kernel(None);
    let action = prepare_tool_action(&kernel, "id", "Boundary", json!({"value": "safe"}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    let replacement_calls = Arc::new(AtomicUsize::new(0));
    kernel.register_tool(Box::new(BoundaryTool {
        calls: replacement_calls.clone(),
    }));

    let error = execute_tool(&kernel, action, &context(), &AgentMode::Act)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("integrity mismatch"));
    assert_eq!(old_calls.load(Ordering::SeqCst), 0);
    assert_eq!(replacement_calls.load(Ordering::SeqCst), 0);
}
