use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_secret_client::{IpcBudget, SecretClient, SecretError, SecretResult};
use loopal_tool_api::{NoopProtectedEffectAudit, ToolContext};
use secrecy::SecretString;

use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;

pub const CANARY: &str = "bg-secret-CANARY-8127349";

struct SecretStore;

#[async_trait]
impl SecretClient for SecretStore {
    async fn get(&self, name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        if name == "token" {
            Ok(SecretString::from(CANARY.to_string()))
        } else {
            Err(SecretError::SecretNotFound(name.to_string()))
        }
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(vec!["token".into()])
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

pub fn test_context(session: &str) -> (Kernel, ToolContext) {
    let kernel = Kernel::new(Settings::default()).unwrap();
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        session,
    );
    let ctx = ToolContext::new(backend, session)
        .with_secret_client(Arc::new(SecretStore))
        .with_protected_effect_audit(Arc::new(NoopProtectedEffectAudit));
    (kernel, ctx)
}

pub async fn run(
    kernel: &Kernel,
    ctx: &ToolContext,
    id: &str,
    name: &str,
    input: serde_json::Value,
) -> loopal_tool_api::ToolResult {
    let action = prepare_tool_action(kernel, id, name, input)
        .await
        .unwrap()
        .into_prepared()
        .unwrap();
    execute_tool(kernel, action, ctx, &AgentMode::Act)
        .await
        .unwrap()
}

pub fn field(content: &str, prefix: &str) -> String {
    content
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("missing {prefix:?} in {content:?}"))
        .to_string()
}
