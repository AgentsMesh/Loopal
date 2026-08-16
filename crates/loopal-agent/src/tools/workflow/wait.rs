use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_protocol::WorkflowWaitRequest;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

pub(super) struct WorkflowWaitTool;

#[async_trait]
impl Tool for WorkflowWaitTool {
    fn name(&self) -> &str {
        "workflow_wait"
    }

    fn description(&self) -> &str {
        "Wait for a workflow revision change or terminal state using a bounded Hub long poll."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        super::schema::wait()
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let request: WorkflowWaitRequest = super::common::decode(input)?;
        Ok(super::common::result(
            super::common::client(ctx)?.wait(request).await,
        ))
    }
}
