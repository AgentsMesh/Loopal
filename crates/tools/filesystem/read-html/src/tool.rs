use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

pub struct ReadHtmlTool;

#[async_trait]
impl Tool for ReadHtmlTool {
    fn name(&self) -> &str {
        "ReadHtml"
    }

    fn description(&self) -> &str {
        "Reads an HTML file and converts it to plain text (markdown-like).\n\n\
         Usage:\n\
         - The file_path must be an absolute path to an .html or .htm file.\n\
         - HTML tags are stripped and content is converted to readable plain text.\n\
         - Useful for extracting text content from web pages saved locally."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["file_path"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the HTML file"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "file_path is required".into(),
            ))
        })?;

        let path = match ctx.backend.resolve_path(file_path, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("html") && !ext.eq_ignore_ascii_case("htm") {
            return Ok(ToolResult::error(
                "ReadHtml only supports .html/.htm files. Use Read for other file types.",
            ));
        }

        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Failed to read {}: {e}",
                    path.display()
                )));
            }
        };

        let converted = html2text::from_read(raw.as_bytes(), 120);
        Ok(ToolResult::success(converted))
    }
}
