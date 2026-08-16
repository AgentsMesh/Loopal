use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
use loopal_protocol::{
    WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
    event_id::scope_turn,
};
use loopal_tool_api::{PermissionDecision, PermissionMode};

use super::support::{Fixture, PermissionBehavior, make_fixture};
use crate::agent_loop::cancel::TurnCancel;
use crate::agent_loop::tools_check_one::CheckOne;
use crate::agent_loop::{StreamingToolHandle, TurnContext};
use crate::tool_action::PreparedToolAction;

pub(super) fn causation() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_permission_coverage"),
        node_id: WorkflowNodeId::new("node_permission_coverage"),
        attempt_id: WorkflowAttemptId::new("watt_permission_coverage"),
    }
}

pub(super) async fn execute_one(
    fixture: &mut Fixture,
    id: &str,
    name: &str,
    input: serde_json::Value,
) -> loopal_error::Result<()> {
    fixture
        .runner
        .start_turn_record(loopal_turn::TurnTrigger::Resume)
        .unwrap();
    let cancel = TurnCancel::new(
        fixture.runner.interrupt.clone(),
        fixture.runner.interrupt_tx.clone(),
    );
    let mut turn = TurnContext::new(1, cancel);
    scope_turn(
        1,
        fixture.runner.execute_tools(
            &mut turn,
            vec![(id.into(), name.into(), input)],
            StreamingToolHandle::empty(),
        ),
    )
    .await
    .map(|_| ())
}

#[tokio::test]
async fn workflow_permissions_require_and_accept_exact_hub_receipts() {
    let mut fixture = make_fixture();
    fixture.runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    fixture.runner.params.workflow_permission_causation = Some(causation());
    fixture
        .frontend
        .set_permission(PermissionBehavior::AllowWithValidReceipt);
    assert_eq!(
        fixture
            .runner
            .check_permission("write-valid", "Write", &serde_json::json!({}))
            .await
            .unwrap(),
        PermissionDecision::Allow
    );

    let mut fixture = make_fixture();
    fixture.runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    fixture.runner.params.workflow_permission_causation = Some(causation());
    fixture
        .frontend
        .set_permission(PermissionBehavior::AllowWithoutReceipt);
    let error = fixture
        .runner
        .check_permission("write-missing", "Write", &serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("missing Hub permission receipt"));

    let mut fixture = make_fixture();
    fixture.runner.params.config.permission_mode = PermissionMode::Bypass;
    fixture.runner.params.workflow_permission_causation = Some(causation());
    fixture
        .frontend
        .set_permission(PermissionBehavior::AllowWithoutReceipt);
    assert_eq!(
        fixture
            .runner
            .check_permission("read-causal", "Read", &serde_json::json!({}))
            .await
            .unwrap(),
        PermissionDecision::Allow
    );
}

#[tokio::test]
async fn workflow_batch_applies_valid_receipts_and_rejects_missing_ones() {
    let mut valid = make_fixture();
    valid.runner.params.config.permission_mode = PermissionMode::Bypass;
    valid.runner.params.workflow_permission_causation = Some(causation());
    valid
        .frontend
        .set_permission(PermissionBehavior::AllowWithValidReceipt);
    let target = valid.temp.path().join("valid-receipt.txt");
    execute_one(
        &mut valid,
        "write-valid",
        "Write",
        serde_json::json!({"file_path": target, "content": "written"}),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "written");

    let mut missing = make_fixture();
    missing.runner.params.config.permission_mode = PermissionMode::Bypass;
    missing.runner.params.workflow_permission_causation = Some(causation());
    let target = missing.temp.path().join("missing-receipt.txt");
    let error = execute_one(
        &mut missing,
        "write-missing",
        "Write",
        serde_json::json!({"file_path": target, "content": "blocked"}),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("missing Hub permission receipt"));
    assert!(!target.exists());
}

#[tokio::test]
async fn policy_allows_causal_reads_and_explicitly_approved_sandbox_writes() {
    let mut read = make_fixture();
    read.runner.params.config.permission_mode = PermissionMode::Bypass;
    read.runner.params.workflow_permission_causation = Some(causation());
    let source = read.temp.path().join("source.txt");
    std::fs::write(&source, "readable").unwrap();
    execute_one(
        &mut read,
        "read",
        "Read",
        serde_json::json!({"file_path": source}),
    )
    .await
    .unwrap();

    let mut write = make_fixture();
    write.runner.params.config.permission_mode = PermissionMode::Bypass;
    let cwd = write.temp.path().to_path_buf();
    let target = cwd.join("blocked.txt");
    write.runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        cwd.clone(),
        Some(ResolvedPolicy {
            policy: SandboxPolicy::DefaultWrite,
            writable_paths: vec![cwd],
            deny_write_globs: vec!["**/blocked.txt".into()],
            deny_read_globs: vec![],
            network: NetworkPolicy::default(),
        }),
        loopal_backend::ResourceLimits::default(),
        "sandbox-policy-allow",
    );
    execute_one(
        &mut write,
        "write",
        "Write",
        serde_json::json!({"file_path": target, "content": "approved"}),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(target).unwrap(), "approved");
}

#[tokio::test]
async fn check_one_reports_original_invalid_input_and_empty_finalize_is_a_noop() {
    let fixture = make_fixture();
    let tool = fixture.runner.params.deps.kernel.get_tool("Write").unwrap();
    let action = PreparedToolAction::new(
        "invalid".into(),
        "Write".into(),
        serde_json::json!({}),
        tool,
        false,
    );
    assert!(matches!(
        fixture.runner.check_one_tool(action).await.unwrap(),
        CheckOne::Denied(_)
    ));

    let mut fixture = make_fixture();
    assert_eq!(
        fixture
            .runner
            .finalize_tool_results(Vec::new())
            .await
            .unwrap(),
        0
    );
}
