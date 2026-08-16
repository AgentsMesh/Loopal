use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_config::{HookConfig, HookEvent, Settings};
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

struct RecordingTool {
    calls: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<Value>>>,
    schema_changed: Arc<std::sync::atomic::AtomicBool>,
    precheck_changed: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait]
impl Tool for RecordingTool {
    fn name(&self) -> &str {
        "Recording"
    }

    fn description(&self) -> &str {
        "Records validated test input"
    }

    fn parameters_schema(&self) -> Value {
        if self.schema_changed.load(Ordering::SeqCst) {
            json!({
                "type": "object",
                "properties": {"other": {"type": "string"}},
                "required": ["other"],
                "additionalProperties": false
            })
        } else {
            json!({
                "type": "object",
                "properties": {"value": {"type": "string"}},
                "required": ["value"],
                "additionalProperties": false
            })
        }
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn precheck(&self, input: &Value) -> Option<String> {
        (self.precheck_changed.load(Ordering::SeqCst) || input["value"] == "blocked")
            .then(|| "blocked value".into())
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push(input);
        Ok(ToolResult::success("recorded"))
    }
}

fn hook(command: String) -> HookConfig {
    HookConfig {
        event: HookEvent::PreToolUse,
        command,
        tool_filter: Some(vec!["Recording".into()]),
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }
}

type TestKernel = (
    Kernel,
    Arc<AtomicUsize>,
    Arc<Mutex<Vec<Value>>>,
    Arc<std::sync::atomic::AtomicBool>,
    Arc<std::sync::atomic::AtomicBool>,
);

fn kernel(command: Option<String>) -> TestKernel {
    let settings = Settings {
        hooks: command.into_iter().map(hook).collect(),
        ..Default::default()
    };
    let kernel = Kernel::new(settings).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let schema_changed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let precheck_changed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    kernel.register_tool(Box::new(RecordingTool {
        calls: Arc::clone(&calls),
        seen: Arc::clone(&seen),
        schema_changed: Arc::clone(&schema_changed),
        precheck_changed: Arc::clone(&precheck_changed),
    }));
    (kernel, calls, seen, schema_changed, precheck_changed)
}

fn context() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "action-integrity",
    );
    ToolContext::new(backend, "action-integrity")
        .with_protected_effect_audit(Arc::new(loopal_tool_api::NoopProtectedEffectAudit))
}

fn rewriting_hook(counter: &std::path::Path, value: &str) -> String {
    let output = r#"{"updated_input":{"value":"VALUE"}}"#.replace("VALUE", value);
    format!(
        "n=0; [ -f '{path}' ] && n=$(cat '{path}'); n=$((n+1)); printf %s $n > '{path}'; printf '%s' '{output}'",
        path = counter.display()
    )
}

#[tokio::test]
async fn rewrite_is_prepared_once_and_executed() {
    let counter = std::env::temp_dir().join(format!("loopal-hook-count-{}", std::process::id()));
    let _ = std::fs::remove_file(&counter);
    let (kernel, calls, seen, _, _) = kernel(Some(rewriting_hook(&counter, "rewritten")));
    let action = prepare_tool_action(&kernel, "id", "Recording", json!({"value": "original"}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    execute_tool(&kernel, action, &context(), &AgentMode::Act)
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&counter).unwrap(), "1");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        &[json!({"value": "rewritten"})]
    );
    let _ = std::fs::remove_file(counter);
}

#[tokio::test]
async fn invalid_rewrite_is_denied_without_execution() {
    let (kernel, calls, _, _, _) = kernel(Some("printf '%s' '{\"updated_input\":{}}'".into()));
    let action = prepare_tool_action(&kernel, "id", "Recording", json!({"value": "valid"}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    let error = execute_tool(&kernel, action, &context(), &AgentMode::Act)
        .await
        .expect_err("invalid rewritten input must fail at the effect boundary");
    assert!(error.to_string().contains("schema validation failed"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn post_approval_precheck_change_is_blocked() {
    let (kernel, calls, _, _, precheck_changed) = kernel(None);
    let action = prepare_tool_action(&kernel, "id", "Recording", json!({"value": "approved"}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    precheck_changed.store(true, Ordering::SeqCst);
    let error = execute_tool(&kernel, action, &context(), &AgentMode::Act)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("tool precheck failed"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn post_approval_schema_change_is_blocked() {
    let (kernel, calls, _, schema_changed, _) = kernel(None);
    let action = prepare_tool_action(&kernel, "id", "Recording", json!({"value": "approved"}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    schema_changed.store(true, Ordering::SeqCst);
    assert!(
        execute_tool(&kernel, action, &context(), &AgentMode::Act)
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
