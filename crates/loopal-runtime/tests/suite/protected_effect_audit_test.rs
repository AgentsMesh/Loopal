use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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

struct EffectTool {
    permission: PermissionLevel,
    calls: Arc<AtomicUsize>,
    order: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl Tool for EffectTool {
    fn name(&self) -> &str {
        "Effect"
    }

    fn description(&self) -> &str {
        "Test effect"
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
        self.permission
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        self.order.lock().unwrap().push("execute");
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolResult::success("done"))
    }
}

struct RecordingAudit {
    fail: bool,
    calls: AtomicUsize,
    requests: Mutex<Vec<ProtectedEffectAuditRequest>>,
    order: Arc<Mutex<Vec<&'static str>>>,
}

impl RecordingAudit {
    fn new(fail: bool, order: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            fail,
            calls: AtomicUsize::new(0),
            requests: Mutex::new(Vec::new()),
            order,
        }
    }
}

#[async_trait]
impl ProtectedEffectAudit for RecordingAudit {
    async fn record(&self, request: &ProtectedEffectAuditRequest) -> loopal_error::Result<()> {
        self.order.lock().unwrap().push("audit");
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.requests.lock().unwrap().push(request.clone());
        self.finish()
    }

    async fn record_permission_decision(
        &self,
        _: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()> {
        self.finish()
    }
}

impl RecordingAudit {
    fn finish(&self) -> loopal_error::Result<()> {
        if self.fail {
            Err(LoopalError::Other("fsync failed".into()))
        } else {
            Ok(())
        }
    }
}

fn fixture(
    permission: PermissionLevel,
) -> (Kernel, Arc<AtomicUsize>, Arc<Mutex<Vec<&'static str>>>) {
    let kernel = Kernel::new(Settings::default()).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let order = Arc::new(Mutex::new(Vec::new()));
    kernel.register_tool(Box::new(EffectTool {
        permission,
        calls: calls.clone(),
        order: order.clone(),
    }));
    (kernel, calls, order)
}

fn context(audit: Option<Arc<dyn ProtectedEffectAudit>>) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "protected-audit",
    );
    let mut context = ToolContext::new(backend, "protected-audit");
    context.protected_effect_audit = audit;
    context
}

async fn prepared(kernel: &Kernel) -> loopal_runtime::tool_action::PreparedToolAction {
    prepare_tool_action(
        kernel,
        "effect-id",
        "Effect",
        json!({"value": "must-not-reach-audit"}),
    )
    .await
    .unwrap()
    .into_prepared()
    .unwrap()
}

#[tokio::test]
async fn write_and_dangerous_execute_only_after_exact_audit_ack() {
    for permission in [PermissionLevel::Write, PermissionLevel::Dangerous] {
        let (kernel, calls, order) = fixture(permission);
        let audit = Arc::new(RecordingAudit::new(false, order.clone()));
        let action = prepared(&kernel).await;
        execute_tool(
            &kernel,
            action,
            &context(Some(audit.clone())),
            &AgentMode::Act,
        )
        .await
        .unwrap();

        assert_eq!(*order.lock().unwrap(), ["audit", "execute"]);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let requests = audit.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].tool_call_id(), "effect-id");
        assert_eq!(requests[0].tool_name(), "Effect");
        let wire = serde_json::to_value(&requests[0]).unwrap();
        assert_eq!(wire.as_object().unwrap().len(), 4);
        assert!(!wire.to_string().contains("must-not-reach-audit"));
    }
}

#[tokio::test]
async fn audit_failure_prevents_protected_effect() {
    let (kernel, calls, order) = fixture(PermissionLevel::Write);
    let audit = Arc::new(RecordingAudit::new(true, order.clone()));
    let error = execute_tool(
        &kernel,
        prepared(&kernel).await,
        &context(Some(audit.clone())),
        &AgentMode::Act,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("protected effect audit failed"));
    assert_eq!(*order.lock().unwrap(), ["audit"]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(audit.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn missing_audit_capability_prevents_protected_effect() {
    let (kernel, calls, order) = fixture(PermissionLevel::Write);
    let error = execute_tool(
        &kernel,
        prepared(&kernel).await,
        &context(None),
        &AgentMode::Act,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("audit capability unavailable"));
    assert!(order.lock().unwrap().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
