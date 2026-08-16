use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_secret_client::{IpcBudget, SecretClient, SecretError, SecretResult};
use loopal_tool_api::ToolContext;
use secrecy::SecretString;
use serde_json::json;

struct MissingSecret;

struct DeniedSecret;

#[async_trait]
impl SecretClient for MissingSecret {
    async fn get(&self, name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Err(SecretError::SecretNotFound(name.into()))
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(Vec::new())
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

#[async_trait]
impl SecretClient for DeniedSecret {
    async fn get(&self, _name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Err(SecretError::PermissionDenied)
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        Err(SecretError::PermissionDenied)
    }

    async fn expand_author(
        &self,
        _template: &str,
        _budget: IpcBudget,
    ) -> SecretResult<SecretString> {
        Err(SecretError::PermissionDenied)
    }

    async fn expand_wire(&self, _template: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Err(SecretError::PermissionDenied)
    }
}

fn context() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "unresolved-secret",
    );
    ToolContext::new(backend, "unresolved-secret")
        .with_protected_effect_audit(Arc::new(loopal_tool_api::NoopProtectedEffectAudit))
}

async fn bash_action_at(
    kernel: &Kernel,
    marker: &str,
) -> loopal_runtime::tool_action::PreparedToolAction {
    prepare_tool_action(
        kernel,
        "id",
        "Bash",
        json!({
            "command": format!("printf effect > {marker}"),
            "env": {"TOKEN": "<secret_ref:missing>"}
        }),
    )
    .await
    .unwrap()
    .into_prepared()
    .unwrap()
}

async fn bash_action(kernel: &Kernel) -> loopal_runtime::tool_action::PreparedToolAction {
    bash_action_at(kernel, "/tmp/loopal-unresolved-secret-effect").await
}

#[tokio::test]
async fn unresolved_wire_ref_fails_closed_but_missing_marker_can_execute() {
    let path = std::path::Path::new("/tmp/loopal-unresolved-secret-effect");
    let _ = std::fs::remove_file(path);
    let kernel = Kernel::new(Settings::default()).unwrap();
    let action = bash_action(&kernel).await;
    let error = execute_tool(&kernel, action, &context(), &AgentMode::Act)
        .await
        .expect_err("missing secret client must fail closed");
    assert!(error.to_string().contains("secret resolution failed"));
    assert!(!path.exists());

    let action = bash_action(&kernel).await;
    let ctx = context().with_secret_client(Arc::new(MissingSecret));
    let result = execute_tool(&kernel, action, &ctx, &AgentMode::Act)
        .await
        .expect("resolved missing-secret marker is safe literal input");
    assert!(!result.is_error);
    assert!(path.exists());
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn denied_secret_resolution_cannot_execute_the_effect() {
    let path = std::path::Path::new("/tmp/loopal-denied-secret-effect");
    let _ = std::fs::remove_file(path);
    let kernel = Kernel::new(Settings::default()).unwrap();
    let action = bash_action_at(&kernel, path.to_str().unwrap()).await;
    let ctx = context().with_secret_client(Arc::new(DeniedSecret));

    let error = execute_tool(&kernel, action, &ctx, &AgentMode::Act)
        .await
        .expect_err("permission-denied secret resolution must fail closed");

    assert!(error.to_string().contains("secret resolution failed"));
    assert!(!path.exists());
}
