use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_protocol::{PermissionDecisionAuditRequest, ProtectedEffectAuditRequest};
use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_secret_client::{IpcBudget, SecretClient, SecretResult, SecretString};
use loopal_tool_api::{PermissionLevel, ProtectedEffectAudit, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

struct OrderedTool(Arc<Mutex<Vec<&'static str>>>);

#[async_trait]
impl Tool for OrderedTool {
    fn name(&self) -> &str {
        "OrderedEffect"
    }

    fn description(&self) -> &str {
        "Protected secret consumer"
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

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &["value"]
    }

    async fn execute(&self, input: Value, _: &ToolContext) -> loopal_error::Result<ToolResult> {
        self.0.lock().unwrap().push("execute");
        assert_eq!(input["value"], "exact-plaintext");
        Ok(ToolResult::success("exact-plaintext"))
    }
}

struct OrderedAudit {
    fail: bool,
    order: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl ProtectedEffectAudit for OrderedAudit {
    async fn record(&self, _: &ProtectedEffectAuditRequest) -> loopal_error::Result<()> {
        self.record_permission_decision_inner()
    }

    async fn record_permission_decision(
        &self,
        _: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()> {
        self.record_permission_decision_inner()
    }
}

impl OrderedAudit {
    fn record_permission_decision_inner(&self) -> loopal_error::Result<()> {
        self.order.lock().unwrap().push("audit");
        if self.fail {
            Err(loopal_error::LoopalError::Other("audit failed".into()))
        } else {
            Ok(())
        }
    }
}

struct OrderedSecrets(Arc<Mutex<Vec<&'static str>>>);

#[async_trait]
impl SecretClient for OrderedSecrets {
    async fn get(&self, name: &str, _: IpcBudget) -> SecretResult<SecretString> {
        self.0.lock().unwrap().push("secret_get");
        assert_eq!(name, "token");
        Ok(SecretString::from("exact-plaintext"))
    }

    async fn list_names(&self, _: IpcBudget) -> SecretResult<Vec<String>> {
        unreachable!()
    }

    async fn expand_author(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }

    async fn expand_wire(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }
}

fn fixture(fail_audit: bool) -> (Kernel, ToolContext, Arc<Mutex<Vec<&'static str>>>) {
    let order = Arc::new(Mutex::new(Vec::new()));
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(OrderedTool(order.clone())));
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "audit-secret-order",
    );
    let context = ToolContext::new(backend, "audit-secret-order")
        .with_protected_effect_audit(Arc::new(OrderedAudit {
            fail: fail_audit,
            order: order.clone(),
        }))
        .with_secret_client(Arc::new(OrderedSecrets(order.clone())));
    (kernel, context, order)
}

async fn action(kernel: &Kernel) -> loopal_runtime::tool_action::PreparedToolAction {
    prepare_tool_action(
        kernel,
        "ordered",
        "OrderedEffect",
        json!({"value": "<secret_ref:token>"}),
    )
    .await
    .unwrap()
    .into_prepared()
    .unwrap()
}

#[tokio::test]
async fn audit_ack_precedes_secret_fetch_and_effect() {
    let (kernel, context, order) = fixture(false);
    let result = execute_tool(&kernel, action(&kernel).await, &context, &AgentMode::Act)
        .await
        .unwrap();
    assert_eq!(*order.lock().unwrap(), ["audit", "secret_get", "execute"]);
    assert_eq!(result.content, "<secret_ref:token>");
}

#[tokio::test]
async fn audit_failure_prevents_secret_fetch() {
    let (kernel, context, order) = fixture(true);
    let error = execute_tool(&kernel, action(&kernel).await, &context, &AgentMode::Act)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("protected effect audit failed"));
    assert_eq!(*order.lock().unwrap(), ["audit"]);
}
