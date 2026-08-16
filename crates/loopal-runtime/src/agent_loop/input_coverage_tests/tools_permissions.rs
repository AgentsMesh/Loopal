use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, PermissionMode, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use super::permission::{causation, execute_one};
use super::support::{PermissionBehavior, make_fixture};
use crate::agent_loop::tools_check_one::CheckOne;
use crate::tool_action::PreparedToolAction;

struct PermissiveWrite;

#[async_trait]
impl Tool for PermissiveWrite {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        "schema permits the reserved-key defense to run"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string"},
                "sandbox_approval_reason": {"type": "string"}
            },
            "required": ["file_path"]
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        unreachable!("admission denial must prevent execution")
    }
}

fn backend(
    cwd: &std::path::Path,
    deny_read: Vec<String>,
    deny_write: Vec<String>,
) -> Arc<dyn loopal_tool_api::Backend> {
    loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        Some(ResolvedPolicy {
            policy: SandboxPolicy::DefaultWrite,
            writable_paths: vec![cwd.to_path_buf()],
            deny_write_globs: deny_write,
            deny_read_globs: deny_read,
            network: NetworkPolicy::default(),
        }),
        loopal_backend::ResourceLimits::default(),
        "tool-permission-branches",
    )
}

#[tokio::test]
async fn sandbox_upgraded_read_only_action_accepts_a_bound_receipt() {
    let mut fixture = make_fixture();
    fixture.runner.params.config.permission_mode = PermissionMode::AskDangerous;
    fixture.runner.params.workflow_permission_causation = Some(causation());
    fixture
        .frontend
        .set_permission(PermissionBehavior::AllowWithValidReceipt);
    let source = fixture.temp.path().join("guarded.txt");
    std::fs::write(&source, "read after approval").unwrap();
    fixture.runner.tool_ctx.backend =
        backend(fixture.temp.path(), vec!["**/guarded.txt".into()], vec![]);

    execute_one(
        &mut fixture,
        "guarded-read",
        "Read",
        json!({"file_path": source}),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn sandbox_annotation_collision_is_denied_by_the_second_boundary() {
    let mut fixture = make_fixture();
    fixture.runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    fixture.runner.tool_ctx.backend =
        backend(fixture.temp.path(), vec![], vec!["**/blocked.txt".into()]);
    let tool: Arc<dyn Tool> = Arc::new(PermissiveWrite);
    let action = PreparedToolAction::new(
        "collision".into(),
        "Write".into(),
        json!({
            "file_path": fixture.temp.path().join("blocked.txt"),
            "sandbox_approval_reason": "model supplied"
        }),
        tool,
        false,
    );

    assert!(matches!(
        fixture.runner.check_one_tool(action).await.unwrap(),
        CheckOne::Denied(_)
    ));
}
