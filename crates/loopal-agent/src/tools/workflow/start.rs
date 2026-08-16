use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_protocol::WorkflowStartRequest;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

pub(super) struct WorkflowStartTool;

#[async_trait]
impl Tool for WorkflowStartTool {
    fn name(&self) -> &str {
        "workflow_start"
    }

    fn description(&self) -> &str {
        "Start a validated, bounded static Agent DAG owned by the Hub. The request_id must be stable across retries."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        super::schema::start()
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
        let request: WorkflowStartRequest = super::common::decode(input)?;
        let client = super::common::client(ctx)?;
        Ok(super::common::start_result(
            client.start_with_confirmation(request).await,
        ))
    }
}
