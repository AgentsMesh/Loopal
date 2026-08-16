use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_protocol::WorkflowGetRequest;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

pub(super) struct WorkflowGetTool;

#[async_trait]
impl Tool for WorkflowGetTool {
    fn name(&self) -> &str {
        "workflow_get"
    }

    fn description(&self) -> &str {
        "Get the authoritative snapshot of one workflow owned by this root session."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        super::schema::get()
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
        let request: WorkflowGetRequest = super::common::decode(input)?;
        Ok(super::common::result(
            super::common::client(ctx)?.get(request).await,
        ))
    }
}
