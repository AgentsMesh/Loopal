use async_trait::async_trait;
use loopal_edit_core::multi_edit::{MultiEditError, MultiEditItem, apply_multi_edits};
use loopal_edit_core::omission_detector::detect_omissions;
use loopal_edit_core::omission_message::format_omission_error;
use loopal_error::LoopalError;
use loopal_secret_runtime::{SECRET_REJECTION_MESSAGE, WIRE_REF_MARKER};
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct MultiEditTool;

/// JSON-schema-bound input type from LLM tool calls. Converted to
/// `loopal_edit_core::multi_edit::MultiEditItem` (same shape, no
/// `Deserialize`/`JsonSchema`) for the pure algorithm in edit-core.
#[derive(Deserialize, JsonSchema)]
pub struct EditItem {
    pub old_string: String,
    pub new_string: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct MultiEditParams {
    pub file_path: String,
    pub edits: Vec<EditItem>,
}

#[async_trait]
impl TypedTool<MultiEditParams> for MultiEditTool {
    fn name(&self) -> &str {
        "MultiEdit"
    }

    fn description(&self) -> &str {
        "Apply multiple sequential edits to a single file atomically. \
         All edits succeed or none are applied."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    fn precheck(&self, input: &MultiEditParams) -> Option<String> {
        if input.file_path.contains(WIRE_REF_MARKER) {
            return Some(SECRET_REJECTION_MESSAGE.into());
        }
        for e in &input.edits {
            if e.old_string.contains(WIRE_REF_MARKER) || e.new_string.contains(WIRE_REF_MARKER) {
                return Some(SECRET_REJECTION_MESSAGE.into());
            }
        }
        None
    }

    async fn execute(
        &self,
        input: MultiEditParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        if input.edits.is_empty() {
            return Ok(ToolResult::error("edits array must not be empty"));
        }

        let path = match ctx.backend.resolve_path(&input.file_path, true) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let content = match ctx.backend.read_raw(&path).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        for (i, edit) in input.edits.iter().enumerate() {
            let omissions = detect_omissions(&edit.new_string);
            if !omissions.is_empty() {
                return Ok(ToolResult::error(format_omission_error(
                    &format!("edit {i} new_string"),
                    &omissions,
                )));
            }
        }

        let items: Vec<MultiEditItem> = input
            .edits
            .iter()
            .map(|e| MultiEditItem {
                old_string: e.old_string.clone(),
                new_string: e.new_string.clone(),
            })
            .collect();

        let outcome = match apply_multi_edits(&content, &items) {
            Ok(o) => o,
            Err(MultiEditError::NotFound { index }) => {
                return Ok(ToolResult::error(format!(
                    "Edit {index}: old_string not found in current content. Earlier edits in \
                     this batch may have already changed that text, or the file changed since \
                     you read it — re-read the file and match the current exact text."
                )));
            }
            Err(MultiEditError::MultipleMatches { index, count }) => {
                return Ok(ToolResult::error(format!(
                    "Edit {index}: old_string found {count} times; must be unique"
                )));
            }
        };

        match ctx.backend.write(&path, &outcome.content).await {
            Ok(_) => Ok(ToolResult::success(format!(
                "Applied {} edit(s) to {}",
                outcome.applied, input.file_path
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
