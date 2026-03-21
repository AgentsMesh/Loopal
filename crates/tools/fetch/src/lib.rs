use async_trait::async_trait;
use futures::StreamExt;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

pub struct FetchTool;

const MAX_BODY_BYTES: usize = 5 * 1024 * 1024; // 5 MB

#[async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str { "Fetch" }

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

    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let url = input["url"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput("url is required".into()))
        })?;

        // Validate URL format before making a network request
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                format!("invalid URL (must start with http:// or https://): {url}"),
            )));
        }

        let prompt = input["prompt"].as_str();

        let response = reqwest::get(url).await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                format!("HTTP request failed: {e}"),
            ))
        })?;

        let status = response.status();
        if !status.is_success() {
            return Ok(ToolResult::error(format!("HTTP {}", status.as_u16())));
        }

        if let Some(cl) = response.content_length()
            && cl > MAX_BODY_BYTES as u64
        {
            return Ok(ToolResult::error(format!(
                "Response too large: {cl} bytes exceeds {} byte limit", MAX_BODY_BYTES
            )));
        }

        let content_type = response.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let ext = extension_from_content_type(&content_type);

        let mut body_bytes = Vec::with_capacity(8192);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                    format!("Failed to read response: {e}"),
                ))
            })?;
            body_bytes.extend_from_slice(&chunk);
            if body_bytes.len() >= MAX_BODY_BYTES {
                body_bytes.truncate(MAX_BODY_BYTES);
                break;
            }
        }

        // With prompt: return content inline (HTML → markdown conversion)
        if let Some(p) = prompt {
            let raw = String::from_utf8_lossy(&body_bytes);
            let converted = if ext == "html" {
                html2text::from_read(raw.as_bytes(), 120)
            } else {
                raw.into_owned()
            };
            let output = format!("[User prompt: {p}]\n\n{converted}");
            return Ok(ToolResult::success(loopal_tool_api::truncate_output(
                &output, 2000, 512_000,
            )));
        }

        // Without prompt: save to temp file
        let size = body_bytes.len();
        let tmp_dir = std::env::temp_dir().join("loopal_fetch");
        std::fs::create_dir_all(&tmp_dir).ok();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let file_path = tmp_dir.join(format!("fetch_{ts}.{ext}"));

        tokio::fs::write(&file_path, &body_bytes).await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                format!("Failed to write temp file: {e}"),
            ))
        })?;

        let path_str = file_path.to_string_lossy();
        Ok(ToolResult::success(format!(
            "Downloaded to: {path_str}\nContent-Type: {content_type}\nSize: {size} bytes"
        )))
    }
}

fn extension_from_content_type(ct: &str) -> &str {
    if ct.contains("text/html") { "html" }
    else if ct.contains("application/pdf") { "pdf" }
    else if ct.contains("image/png") { "png" }
    else if ct.contains("image/jpeg") { "jpg" }
    else if ct.contains("image/svg") { "svg" }
    else if ct.contains("application/json") { "json" }
    else if ct.contains("text/") { "txt" }
    else { "bin" }
}
