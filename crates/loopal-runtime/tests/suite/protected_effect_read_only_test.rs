use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::{PermissionDecisionAuditRequest, ProtectedEffectAuditRequest};
use loopal_runtime::mode::AgentMode;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_tool_api::{PermissionLevel, ProtectedEffectAudit, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

struct ReadOnlyEffect(Arc<AtomicUsize>);

#[async_trait]
impl Tool for ReadOnlyEffect {
    fn name(&self) -> &str {
        "ReadOnlyEffect"
    }

    fn description(&self) -> &str {
        "read-only test effect"
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ToolResult::success("done"))
    }
}

struct FailingAudit(AtomicUsize);

#[async_trait]
impl ProtectedEffectAudit for FailingAudit {
    async fn record(&self, _: &ProtectedEffectAuditRequest) -> loopal_error::Result<()> {
        self.fail()
    }

    async fn record_permission_decision(
        &self,
        _: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()> {
        self.fail()
    }
}

impl FailingAudit {
    fn fail(&self) -> loopal_error::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(LoopalError::Other("must not run".into()))
    }
}

#[tokio::test]
async fn read_only_tool_does_not_require_generic_effect_audit() {
    let kernel = Kernel::new(Settings::default()).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    kernel.register_tool(Box::new(ReadOnlyEffect(calls.clone())));
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "protected-audit-read-only",
    );
    let audit = Arc::new(FailingAudit(AtomicUsize::new(0)));
    let context = ToolContext::new(backend, "protected-audit-read-only")
        .with_protected_effect_audit(audit.clone());
    let action = prepare_tool_action(&kernel, "id", "ReadOnlyEffect", json!({}))
        .await
        .unwrap()
        .into_prepared()
        .unwrap();

    execute_tool(&kernel, action, &context, &AgentMode::Act)
        .await
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(audit.0.load(Ordering::SeqCst), 0);
}
