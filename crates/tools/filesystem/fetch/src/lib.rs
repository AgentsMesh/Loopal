use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

pub struct FetchTool;

#[async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str {
        "Fetch"
    }

    fn description(&self) -> &str {
        "Download a URL. Without prompt: saves to temp file and returns path. \
         With prompt: returns content directly (HTML auto-converted to markdown)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["url"],
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to download"
                },
                "prompt": {
                    "type": "string",
                    "description": "If provided, return content inline (with prompt prepended) instead of saving to file"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let url = input["url"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "url is required".into(),
            ))
        })?;

        // Validate URL format before making a network request
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                format!("invalid URL (must start with http:// or https://): {url}"),
            )));
        }

        let fetch_result = match ctx.backend.fetch(url).await {
            Ok(r) => r,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        if !is_success(fetch_result.status) {
            return Ok(ToolResult::error(format!("HTTP {}", fetch_result.status)));
        }

        let content_type = fetch_result
            .content_type
            .as_deref()
            .unwrap_or("application/octet-stream");
        let ext = extension_from_content_type(content_type);
        let prompt = input["prompt"].as_str();

        // With prompt: return content inline (HTML -> markdown conversion)
        if let Some(p) = prompt {
            let converted = if ext == "html" {
                html2text::from_read(fetch_result.body.as_bytes(), 120)
            } else {
                fetch_result.body
            };
            let output = format!("[User prompt: {p}]\n\n{converted}");
            return Ok(ToolResult::success(loopal_tool_api::truncate_output(
                &output, 2000, 512_000,
            )));
        }

        // Without prompt: save to temp file via backend
        let size = fetch_result.body.len();
        let tmp_dir = std::env::temp_dir().join("loopal_fetch");
        let uuid = simple_uuid();
        let file_path = tmp_dir.join(format!("fetch_{uuid}.{ext}"));

        // Ensure temp directory exists, then write via backend
        if let Err(e) = ctx
            .backend
            .create_dir_all(tmp_dir.to_str().unwrap_or("."))
            .await
        {
            return Ok(ToolResult::error(format!("Failed to create temp dir: {e}")));
        }
        if let Err(e) = ctx
            .backend
            .write(file_path.to_str().unwrap_or("."), &fetch_result.body)
            .await
        {
            return Ok(ToolResult::error(format!("Failed to write temp file: {e}")));
        }

        let path_str = file_path.to_string_lossy();
        Ok(ToolResult::success(format!(
            "Downloaded to: {path_str}\nContent-Type: {content_type}\nSize: {size} bytes"
        )))
    }
}

fn is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

fn extension_from_content_type(ct: &str) -> &str {
    if ct.contains("text/html") {
        "html"
    } else if ct.contains("application/pdf") {
        "pdf"
    } else if ct.contains("image/png") {
        "png"
    } else if ct.contains("image/jpeg") {
        "jpg"
    } else if ct.contains("image/svg") {
        "svg"
    } else if ct.contains("application/json") {
        "json"
    } else if ct.contains("text/") {
        "txt"
    } else {
        "bin"
    }
}

/// Minimal UUID v4 without external dependency (8 hex chars, good enough for temp files).
fn simple_uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let pid = std::process::id();
    format!("{nanos:08x}{pid:08x}")
}
