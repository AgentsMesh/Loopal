mod cleanup;
mod refiner;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub use refiner::__try_refine_internal;

pub struct FetchTool;

#[derive(Deserialize, JsonSchema)]
pub struct FetchParams {
    pub url: String,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[async_trait]
impl TypedTool<FetchParams> for FetchTool {
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

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: FetchParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        if !input.url.starts_with("http://") && !input.url.starts_with("https://") {
            return Err(LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                format!(
                    "invalid URL (must start with http:// or https://): {}",
                    input.url
                ),
            )));
        }

        let fetch_result = match ctx.backend.fetch(&input.url).await {
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
        let redirect_prefix = fetch_result
            .final_url
            .as_deref()
            .map(|u| format!("Final-URL: {u}\n\n"))
            .unwrap_or_default();

        if let Some(p) = &input.prompt {
            let converted = if ext == "html" {
                html2text::from_read(fetch_result.body.as_bytes(), 120)
            } else {
                fetch_result.body
            };

            if let Some(mut refined) = __try_refine_internal(ctx, p, &input.url, &converted).await {
                refined.content = format!("{redirect_prefix}{}", refined.content);
                return Ok(refined);
            }

            let output = format!("{redirect_prefix}[User prompt: {p}]\n\n{converted}");
            return Ok(ToolResult::success(loopal_tool_api::truncate_output(
                &output, 2000, 512_000,
            )));
        }

        let size = fetch_result.body.len();
        let path_str = save_to_tmp(ctx, &fetch_result.body, ext).await?;
        Ok(ToolResult::success(format!(
            "{redirect_prefix}Downloaded to: {path_str}\nContent-Type: {content_type}\nSize: {size} bytes"
        )))
    }
}

pub(crate) async fn save_to_tmp(
    ctx: &ToolContext,
    body: &str,
    ext: &str,
) -> Result<String, LoopalError> {
    let tmp_dir = ctx.backend.cwd().as_path().join(".loopal_fetch");
    cleanup::cleanup_old_files_once(&tmp_dir);
    let uuid = simple_uuid();
    let file_path = tmp_dir.join(format!("fetch_{uuid}.{ext}"));
    let tmp_dir_resolved = match ctx
        .backend
        .resolve_path(tmp_dir.to_str().unwrap_or("."), true)
    {
        Ok(p) => p,
        Err(e) => {
            return Err(LoopalError::Other(format!(
                "Failed to resolve temp dir: {e}"
            )));
        }
    };
    if let Err(e) = ctx.backend.create_dir_all(&tmp_dir_resolved).await {
        return Err(LoopalError::Other(format!(
            "Failed to create temp dir: {e}"
        )));
    }
    let file_path_resolved = match ctx
        .backend
        .resolve_path(file_path.to_str().unwrap_or("."), true)
    {
        Ok(p) => p,
        Err(e) => {
            return Err(LoopalError::Other(format!(
                "Failed to resolve temp file: {e}"
            )));
        }
    };
    if let Err(e) = ctx.backend.write(&file_path_resolved, body).await {
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
