mod commit;
mod plan;

use async_trait::async_trait;
use loopal_edit_core::patch_parser::parse_patch;
use loopal_error::LoopalError;
use loopal_secret_runtime::{SECRET_REJECTION_MESSAGE, WIRE_REF_MARKER};
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct ApplyPatchTool;

#[derive(Deserialize, JsonSchema)]
pub struct ApplyPatchParams {
    pub patch: String,
}

#[async_trait]
impl TypedTool<ApplyPatchParams> for ApplyPatchTool {
    fn name(&self) -> &str {
        "ApplyPatch"
    }

    fn description(&self) -> &str {
        "Apply a patch to create, update, or delete multiple files in a best-effort batch \
         (fail-fast on first error; already-applied files are reported in the error). \
         Uses a unified diff-like format with context lines for reliable matching."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    fn precheck(&self, input: &ApplyPatchParams) -> Option<String> {
        if input.patch.contains(WIRE_REF_MARKER) {
            return Some(SECRET_REJECTION_MESSAGE.into());
        }
        None
    }

    async fn execute(
        &self,
        input: ApplyPatchParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let ops = match parse_patch(&input.patch) {
            Ok(o) => o,
            Err(e) => return Ok(ToolResult::error(format!("parse error: {e}"))),
        };
        if ops.is_empty() {
            return Ok(ToolResult::error("patch contains no file operations"));
        }

        let batch_ops = match plan::plan_patch(&ops, ctx).await {
            Ok(b) => b,
            Err(e) => return Ok(ToolResult::error(e.message)),
        };

        match commit::commit_patch(batch_ops, ctx).await {
            Ok(result) => Ok(result),
            Err(msg) => Ok(ToolResult::error(msg)),
        }
    }
}
