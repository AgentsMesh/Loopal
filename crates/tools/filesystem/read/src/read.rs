use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "Read a file from the filesystem. Returns content with line numbers. \
         Supports PDF (text extraction) and HTML (converts to markdown)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["file_path"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-based)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                },
                "pages": {
                    "type": "string",
                    "description": "Page range for PDF files (e.g., '1-5', '3')"
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
        let pages = input["pages"].as_str().map(|s| s.to_string());

        // Resolve path (checks traversal for relative paths)
        let path = match ctx.backend.resolve_path(file_path, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // PDF handling (sync extraction, doesn't use backend)
        if ext.eq_ignore_ascii_case("pdf") {
            return match crate::read_pdf::extract_pdf_text(&path, pages.as_deref()) {
                Ok(text) => Ok(ToolResult::success(text)),
                Err(e) => Ok(ToolResult::error(e)),
            };
        }

        if pages.is_some() {
            return Ok(ToolResult::error(
                "pages parameter is only supported for PDF files",
            ));
        }

        // HTML handling — sync read + convert to plain text/markdown
        if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") {
            return read_html(&path);
        }

        // Text file via backend (handles size check, binary detection, line numbering)
        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().unwrap_or(2000) as usize;

        match ctx.backend.read(file_path, offset - 1, limit).await {
            Ok(result) => Ok(ToolResult::success(result.content)),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

fn read_html(path: &std::path::Path) -> Result<ToolResult, LoopalError> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(format!(
            "Failed to read {}: {e}",
            path.display()
        )))
    })?;
    let converted = html2text::from_read(raw.as_bytes(), 120);
    Ok(ToolResult::success(converted))
}
