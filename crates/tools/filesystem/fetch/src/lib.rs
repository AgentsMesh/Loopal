mod cleanup;
mod refiner;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

pub use refiner::__try_refine_internal;

pub struct FetchTool;

#[async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str {
        "Fetch"
    }

    fn description(&self) -> &str {
        "Download a URL and process its content.\n\
         - Without prompt: saves to a temp file and returns the path.\n\
         - With prompt: returns content inline. When the page exceeds the configured \
           threshold, the body is summarized by a fast model against the prompt and \
           the raw markdown is saved to disk for re-reading.\n\
         - WILL FAIL for authenticated/private URLs (Google Docs, Jira, Confluence). \
           Use a specialized MCP tool for those.\n\
         - HTTP URLs auto-upgrade to HTTPS. Includes a 15-minute cache.\n\
         - When a URL redirects to a different host, the tool returns the redirect URL — \
           make a new request with it."
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
                    "description": "What you want extracted from the page. \
                        Large pages are summarized by a fast model against this intent — \
                        be specific (e.g. 'find the API auth header format' beats 'summarize'). \
                        Without a prompt, the page is saved to a temp file."
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

        if let Some(p) = prompt {
            let converted = if ext == "html" {
                html2text::from_read(fetch_result.body.as_bytes(), 120)
            } else {
                fetch_result.body
            };

            if let Some(refined) = __try_refine_internal(ctx, p, url, &converted).await {
                return Ok(refined);
            }

            let output = format!("[User prompt: {p}]\n\n{converted}");
            return Ok(ToolResult::success(loopal_tool_api::truncate_output(
                &output, 2000, 512_000,
            )));
        }

        let size = fetch_result.body.len();
        let path_str = save_to_tmp(ctx, &fetch_result.body, ext).await?;
        Ok(ToolResult::success(format!(
            "Downloaded to: {path_str}\nContent-Type: {content_type}\nSize: {size} bytes"
        )))
    }
}

/// Internal helper used by `Tool::execute` and by `refiner::__try_refine_internal`.
pub(crate) async fn save_to_tmp(
    ctx: &ToolContext,
    body: &str,
    ext: &str,
) -> Result<String, LoopalError> {
    let tmp_dir = ctx.backend.cwd().join(".loopal_fetch");
    cleanup::cleanup_old_files_once(&tmp_dir);
    let uuid = simple_uuid();
    let file_path = tmp_dir.join(format!("fetch_{uuid}.{ext}"));
    if let Err(e) = ctx
        .backend
        .create_dir_all(tmp_dir.to_str().unwrap_or("."))
        .await
    {
        return Err(LoopalError::Other(format!(
            "Failed to create temp dir: {e}"
        )));
    }
    if let Err(e) = ctx
        .backend
        .write(file_path.to_str().unwrap_or("."), body)
        .await
    {
        return Err(LoopalError::Other(format!(
            "Failed to write temp file: {e}"
        )));
    }
    Ok(file_path.to_string_lossy().into_owned())
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

fn simple_uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let pid = std::process::id();
    format!("{nanos:08x}{pid:08x}")
}
