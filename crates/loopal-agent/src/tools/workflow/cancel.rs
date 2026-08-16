use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_protocol::WorkflowCancelRequest;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

pub(super) struct WorkflowCancelTool;

#[async_trait]
impl Tool for WorkflowCancelTool {
    fn name(&self) -> &str {
        "workflow_cancel"
    }

    fn description(&self) -> &str {
        "Cancel one workflow owned by this root session. The request_id must be stable across retries."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        super::schema::cancel()
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let request: WorkflowCancelRequest = super::common::decode(input)?;
        Ok(super::common::result(
            super::common::client(ctx)?.cancel(request).await,
        ))
    }
}
