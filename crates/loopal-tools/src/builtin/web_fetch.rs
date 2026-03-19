use async_trait::async_trait;
use futures::StreamExt;
use loopal_types::error::LoopalError;
use loopal_types::permission::PermissionLevel;
use loopal_types::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

use crate::truncate::truncate_output;

pub struct WebFetchTool;

const MAX_BODY_LINES: usize = 2000;
const MAX_BODY_BYTES: usize = 512_000;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL via HTTP GET. Returns the response body text."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["url"],
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let url = input["url"]
            .as_str()
            .ok_or_else(|| {
                LoopalError::Tool(loopal_types::error::ToolError::InvalidInput(
                    "url is required".into(),
                ))
            })?;

        let response = reqwest::get(url).await.map_err(|e| {
            LoopalError::Tool(loopal_types::error::ToolError::ExecutionFailed(
                format!("HTTP request failed: {}", e),
            ))
        })?;

        let status = response.status();

        // Reject responses that advertise a Content-Length larger than 5 MB
        const MAX_CONTENT_LENGTH: u64 = 5 * 1024 * 1024;
        if let Some(cl) = response.content_length()
            && cl > MAX_CONTENT_LENGTH {
                return Ok(ToolResult::error(format!(
                    "Response too large: Content-Length {cl} exceeds {} byte limit",
                    MAX_CONTENT_LENGTH
                )));
            }

        // Read body in chunks, stopping at MAX_BODY_BYTES to avoid unbounded allocation
        let mut body_bytes = Vec::with_capacity(MAX_BODY_BYTES.min(8192));
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                LoopalError::Tool(loopal_types::error::ToolError::ExecutionFailed(
                    format!("Failed to read response body: {}", e),
                ))
            })?;
            body_bytes.extend_from_slice(&chunk);
            if body_bytes.len() >= MAX_BODY_BYTES {
                body_bytes.truncate(MAX_BODY_BYTES);
                break;
            }
        }
        let body = String::from_utf8_lossy(&body_bytes).into_owned();

        let truncated = truncate_output(&body, MAX_BODY_LINES, MAX_BODY_BYTES);

        if status.is_success() {
            Ok(ToolResult::success(truncated))
        } else {
            Ok(ToolResult::error(format!(
                "HTTP {}: {}",
                status.as_u16(),
                truncated
            )))
        }
    }
}
