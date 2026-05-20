use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::dst_resolve::{ensure_parent_dir, resolve_dst};

pub struct CopyFileTool;

#[derive(Deserialize, JsonSchema)]
pub struct CopyFileParams {
    pub src: String,
    pub dst: String,
}

#[async_trait]
impl TypedTool<CopyFileParams> for CopyFileTool {
    fn name(&self) -> &str {
        "CopyFile"
    }

    fn description(&self) -> &str {
        "Copy a file to a new location."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: CopyFileParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let src = match ctx.backend.resolve_path(&input.src, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };
        let src_info = match ctx.backend.file_info(&src).await {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };
        if src_info.is_dir {
            return Ok(ToolResult::error(
                "source must be a file (use Bash for directory copies)",
            ));
        }

        let dst = match resolve_dst(ctx.backend.as_ref(), &input.src, &input.dst).await {
            Ok(d) => d,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        if let Err(e) = ensure_parent_dir(ctx.backend.as_ref(), &dst.resolved).await {
            return Ok(ToolResult::error(e.to_string()));
        }

        match ctx.backend.copy(&src, &dst.resolved).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Copied {} → {}",
                input.src, dst.display
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
