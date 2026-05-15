use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct DeleteTool;

#[derive(Deserialize, JsonSchema)]
pub struct DeleteParams {
    pub path: String,
}

#[async_trait]
impl TypedTool<DeleteParams> for DeleteTool {
    fn name(&self) -> &str {
        "Delete"
    }

    fn description(&self) -> &str {
        "Delete a file or directory."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(
        &self,
        input: DeleteParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let info = match ctx.backend.file_info(&input.path).await {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let kind = if info.is_dir { "directory" } else { "file" };

        match ctx.backend.remove(&input.path).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Deleted {} ({kind})",
                input.path
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
