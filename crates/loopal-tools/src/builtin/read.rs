use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "Read" }

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

    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput("file_path is required".into()))
        })?;

        let path = resolve_path(file_path, &ctx.cwd);
        let pages = input["pages"].as_str().map(|s| s.to_string());

        // Guard against path traversal for relative paths
        if !PathBuf::from(file_path).is_absolute()
            && let Ok(canonical) = path.canonicalize()
            && !canonical.starts_with(&ctx.cwd)
        {
            return Ok(ToolResult::error("path outside working directory"));
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // PDF handling
        if ext.eq_ignore_ascii_case("pdf") {
            return match super::read_pdf::extract_pdf_text(&path, pages.as_deref()) {
                Ok(text) => Ok(ToolResult::success(text)),
                Err(e) => Ok(ToolResult::error(e)),
            };
        }

        if pages.is_some() {
            return Ok(ToolResult::error("pages parameter is only supported for PDF files"));
        }

        // HTML handling — convert to plain text/markdown
        if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") {
            return read_html(&path).await;
        }

        // Default: text file with line numbers
        read_text(&path, &input).await
    }
}

async fn read_html(path: &std::path::Path) -> Result<ToolResult, LoopalError> {
    let raw = tokio::fs::read_to_string(path).await.map_err(|e| {
        LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
            format!("Failed to read {}: {e}", path.display()),
        ))
    })?;
    let converted = html2text::from_read(raw.as_bytes(), 120);
    Ok(ToolResult::success(converted))
}

async fn read_text(path: &std::path::Path, input: &Value) -> Result<ToolResult, LoopalError> {
    let content = tokio::fs::read_to_string(path).await.map_err(|e| {
        LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
            format!("Failed to read {}: {e}", path.display()),
        ))
    })?;

    let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = input["limit"].as_u64().unwrap_or(2000) as usize;

    let lines: Vec<&str> = content.lines().collect();
    let start = (offset - 1).min(lines.len());
    let end = (start + limit).min(lines.len());

    let mut result = String::new();
    for (i, line) in lines[start..end].iter().enumerate() {
        let line_num = start + i + 1;
        result.push_str(&format!("{:>6}\t{}\n", line_num, line));
    }
    Ok(ToolResult::success(result))
}

fn resolve_path(file_path: &str, cwd: &std::path::Path) -> PathBuf {
    let p = PathBuf::from(file_path);
    if p.is_absolute() { p } else { cwd.join(p) }
}
