use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
use loopal_protocol::{
    AgentEvent, AgentEventPayload, PermissionAuditDecision, PermissionAuditSource,
    PermissionDecisionAuditRequest, ProtectedEffectAuditRequest,
};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_tool_api::{PermissionMode, ProtectedEffectAudit};
use tokio::sync::mpsc;

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

#[derive(Default)]
struct FailingAudit {
    decisions: Mutex<Vec<PermissionDecisionAuditRequest>>,
    effects: AtomicUsize,
}

#[async_trait]
impl ProtectedEffectAudit for FailingAudit {
    async fn record(&self, _: &ProtectedEffectAuditRequest) -> loopal_error::Result<()> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Err(loopal_error::LoopalError::Other(
            "protected effect must not run".into(),
        ))
    }

    async fn record_permission_decision(
        &self,
        request: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()> {
        self.decisions.lock().unwrap().push(request.clone());
        Err(loopal_error::LoopalError::Other(
            "audit fsync failed".into(),
        ))
    }
}

struct Fixture {
    runner: AgentLoopRunner,
    events: mpsc::Receiver<AgentEvent>,
    permission_tx: mpsc::Sender<bool>,
    audit: Arc<FailingAudit>,
    target: std::path::PathBuf,
    _cwd: tempfile::TempDir,
}

fn fixture(permission_mode: PermissionMode) -> Fixture {
    let (mut runner, events, _mbox, _control, permission_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = permission_mode;
    let cwd = tempfile::tempdir().unwrap();
    let target = cwd.path().join("blocked.txt");
    let policy = ResolvedPolicy {
        policy: SandboxPolicy::DefaultWrite,
        writable_paths: vec![cwd.path().to_path_buf()],
        deny_write_globs: vec!["**/blocked.txt".into()],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    };
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        cwd.path().to_path_buf(),
        Some(policy),
        loopal_backend::ResourceLimits::default(),
        "permission-audit-failure",
    );
    let audit = Arc::new(FailingAudit::default());
    runner.params.deps.protected_effect_audit = audit.clone();
    assert!(requires_approval(&runner, &target));
    Fixture {
        runner,
        events,
        permission_tx,
        audit,
        target,
        _cwd: cwd,
    }
}

fn requires_approval(runner: &AgentLoopRunner, target: &std::path::Path) -> bool {
    runner
        .tool_ctx
        .backend
        .check_sandbox_path(target.to_str().unwrap(), true)
        .is_some()
}

fn tool_call(id: &str, target: &std::path::Path) -> Vec<(String, String, serde_json::Value)> {
    vec![(
        id.into(),
        "Write".into(),
        serde_json::json!({"file_path": target, "content": "must-not-write"}),
    )]
}

fn assert_failed(fixture: &Fixture, source: PermissionAuditSource, id: &str) {
    let decisions = fixture.audit.decisions.lock().unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].tool_call_id(), id);
    assert_eq!(decisions[0].tool_name(), "Write");
    assert_eq!(decisions[0].decision(), PermissionAuditDecision::Allow);
    assert_eq!(decisions[0].source(), source);
    assert_eq!(fixture.audit.effects.load(Ordering::SeqCst), 0);
    assert!(!fixture.target.exists());
    assert!(requires_approval(&fixture.runner, &fixture.target));
}

#[tokio::test]
async fn policy_allow_audit_failure_prevents_approval_and_execution() {
    let mut fixture = fixture(PermissionMode::Bypass);
    let mut turn = make_turn_ctx();
    let error = in_turn(fixture.runner.execute_tools(
        &mut turn,
        tool_call("policy-write", &fixture.target),
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap_err();
    assert!(error.to_string().contains("permission audit failed"));
    assert_failed(&fixture, PermissionAuditSource::Policy, "policy-write");
    assert_no_tool_events(&mut fixture.events);
}

#[tokio::test]
async fn frontend_allow_audit_failure_prevents_approval_and_execution() {
    let mut fixture = fixture(PermissionMode::AskAnyWrite);
    let target = fixture.target.clone();
    let mut turn = make_turn_ctx();
    let error = {
        let execution = in_turn(fixture.runner.execute_tools(
            &mut turn,
            tool_call("frontend-write", &target),
            loopal_runtime::agent_loop::StreamingToolHandle::empty(),
        ));
        tokio::pin!(execution);
        let input = loop {
            tokio::select! {
                event = fixture.events.recv() => {
                    if let AgentEventPayload::ToolPermissionRequest { input, .. } = event.unwrap().payload {
                        break input;
                    }
                }
                result = &mut execution => panic!("permission request not emitted: {result:?}"),
            }
        };
        assert_eq!(input["file_path"], serde_json::json!(target));
        assert!(input["sandbox_approval_reason"].as_str().is_some());
        fixture.permission_tx.send(true).await.unwrap();
        execution.await.unwrap_err()
    };
    assert!(error.to_string().contains("permission audit failed"));
    assert_failed(&fixture, PermissionAuditSource::Frontend, "frontend-write");
    assert_no_tool_events(&mut fixture.events);
}

fn assert_no_tool_events(events: &mut mpsc::Receiver<AgentEvent>) {
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(
            event.payload,
            AgentEventPayload::ToolProgress { .. } | AgentEventPayload::ToolResult { .. }
        ));
    }
}
