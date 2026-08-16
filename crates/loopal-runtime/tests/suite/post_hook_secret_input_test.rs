use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::{HookConfig, HookEvent, Settings};
use loopal_kernel::Kernel;
use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_secret_client::{IpcBudget, SecretClient, SecretResult};
use loopal_tool_api::ToolContext;
use secrecy::SecretString;
use serde_json::json;

struct OneSecret;

#[async_trait]
impl SecretClient for OneSecret {
    async fn get(&self, _name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from("post-hook-plaintext-canary"))
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(vec!["canary".into()])
    }

    async fn expand_author(
        &self,
        template: &str,
        _budget: IpcBudget,
    ) -> SecretResult<SecretString> {
        Ok(SecretString::from(template.to_string()))
    }

    async fn expand_wire(&self, template: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from(template.to_string()))
    }
}

#[tokio::test]
async fn post_hook_receives_placeholder_input_not_plaintext() {
    let capture = std::env::temp_dir().join(format!("loopal-post-hook-{}", std::process::id()));
    let _ = std::fs::remove_file(&capture);
    let hook = HookConfig {
        event: HookEvent::PostToolUse,
        command: format!(
            "cat > '{}'; printf '%s\\n' '{{\"additional_context\":\"post-hook-plaintext-canary\"}}'",
            capture.display()
        ),
        tool_filter: Some(vec!["Bash".into()]),
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    };
    let kernel = Kernel::new(Settings {
        hooks: vec![hook],
        ..Default::default()
    })
    .unwrap();
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "post-hook-secret",
    );
    let ctx = ToolContext::new(backend, "post-hook-secret")
        .with_protected_effect_audit(Arc::new(loopal_tool_api::NoopProtectedEffectAudit))
        .with_secret_client(Arc::new(OneSecret));
    let action = prepare_tool_action(
        &kernel,
        "id",
        "Bash",
        json!({
            "command": "printf '%s' \"$TOKEN\"",
            "env": {"TOKEN": "<secret_ref:canary>"}
        }),
    )
    .await
    .unwrap()
    .into_prepared()
    .unwrap();
    let result = execute_tool(&kernel, action, &ctx, &AgentMode::Act)
        .await
        .unwrap();
    assert!(result.content.starts_with("<secret_ref:canary>"));
    assert!(result.content.contains("[POST-HOOK FEEDBACK]"));
    assert!(result.content.contains("<secret_ref:canary>"));
    assert!(!result.content.contains("post-hook-plaintext-canary"));
    let payload = std::fs::read_to_string(&capture).unwrap();
    assert!(payload.contains("<secret_ref:canary>"));
    assert!(!payload.contains("post-hook-plaintext-canary"));
    let _ = std::fs::remove_file(capture);
}
