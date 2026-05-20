use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::dst_resolve::{ensure_parent_dir, resolve_dst};

pub struct MoveFileTool;

#[derive(Deserialize, JsonSchema)]
pub struct MoveFileParams {
    pub src: String,
    pub dst: String,
}

#[async_trait]
impl TypedTool<MoveFileParams> for MoveFileTool {
    fn name(&self) -> &str {
        "MoveFile"
    }

    fn description(&self) -> &str {
        "Move or rename a file or directory."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: MoveFileParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let src = match ctx.backend.resolve_path(&input.src, true) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };
        if let Err(e) = ctx.backend.file_info(&src).await {
            return Ok(ToolResult::error(e.to_string()));
        }

        let dst = match resolve_dst(ctx.backend.as_ref(), &input.src, &input.dst).await {
            Ok(d) => d,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        if let Err(e) = ensure_parent_dir(ctx.backend.as_ref(), &dst.resolved).await {
            return Ok(ToolResult::error(e.to_string()));
        }

        match ctx.backend.rename(&src, &dst.resolved).await {
            Ok(()) => {}
            Err(_) => {
                if let Err(e) = ctx.backend.copy(&src, &dst.resolved).await {
                    return Ok(ToolResult::error(e.to_string()));
                }
                if let Err(e) = ctx.backend.remove(&src).await {
                    return Ok(ToolResult::error(e.to_string()));
                }
            }
        }

        Ok(ToolResult::success(format!(
            "Moved {} → {}",
            input.src, dst.display
        )))
    }
}
