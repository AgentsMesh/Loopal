use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

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
        if let Err(e) = ctx.backend.file_info(&input.src).await {
            return Ok(ToolResult::error(e.to_string()));
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

        match ctx.backend.rename(&input.src, &final_dst).await {
            Ok(()) => {}
            Err(_) => {
                if let Err(e) = ctx.backend.copy(&input.src, &final_dst).await {
                    return Ok(ToolResult::error(e.to_string()));
                }
                if let Err(e) = ctx.backend.remove(&input.src).await {
                    return Ok(ToolResult::error(e.to_string()));
                }
            }
        }

        Ok(ToolResult::success(format!(
            "Moved {} → {final_dst}",
            input.src
        )))
    }
}
