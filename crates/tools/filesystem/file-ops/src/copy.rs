use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

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

    async fn execute(
        &self,
        input: CopyFileParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let src_info = match ctx.backend.file_info(&input.src).await {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };
        if src_info.is_dir {
            return Ok(ToolResult::error(
                "source must be a file (use Bash for directory copies)",
            ));
        }

        let final_dst = match ctx.backend.file_info(&input.dst).await {
            Ok(info) if info.is_dir => {
                let src_path = std::path::Path::new(&input.src);
                let name = src_path.file_name().ok_or_else(|| {
                    LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                        "source has no file name".into(),
                    ))
                })?;
                let dst_path = std::path::Path::new(&input.dst).join(name);
                dst_path.to_string_lossy().into_owned()
            }
            _ => input.dst.clone(),
        };

        if let Some(parent) = std::path::Path::new(&final_dst).parent()
            && let Err(e) = ctx.backend.create_dir_all(&parent.to_string_lossy()).await
        {
            return Ok(ToolResult::error(e.to_string()));
        }

        match ctx.backend.copy(&input.src, &final_dst).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Copied {} → {final_dst}",
                input.src
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
