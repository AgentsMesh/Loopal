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
        "Reads a file from the local filesystem. You can access any file directly by using this tool.\n\
         Assume this tool is able to read all files on the machine. If a path is provided, assume it is valid. \
         It is okay to read a file that does not exist; an error will be returned.\n\n\
         Usage:\n\
         - The file_path parameter must be an absolute path, not a relative path.\n\
         - By default, it reads up to 2000 lines starting from the beginning of the file.\n\
         - You can optionally specify a line offset and limit (especially handy for long files), \
         but it's recommended to read the whole file by not providing these parameters.\n\
         - Results are returned using cat -n format, with line numbers starting at 1.\n\
         - This tool can read images (PNG, JPG, etc). When reading an image file the contents are presented visually.\n\
         - This tool can read PDF files (.pdf). For large PDFs (more than 10 pages), you MUST provide the pages \
         parameter to read specific page ranges (e.g., pages: \"1-5\"). Max 20 pages per request.\n\
         - This tool can read Jupyter notebooks (.ipynb files) and returns all cells with their outputs.\n\
         - Supports HTML files — auto-converted to markdown.\n\
         - This tool can only read files, not directories. To read a directory, use Ls or an ls command via Bash.\n\
         - If the user provides a path to a screenshot, ALWAYS use this tool to view the file at the path.\n\
         - If you read a file that exists but has empty contents you will receive a system reminder warning."
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
