use std::path::Path;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::page_range::parse_page_range;

pub struct ReadPdfTool;

#[derive(Deserialize, JsonSchema)]
pub struct ReadPdfParams {
    pub file_path: String,
    #[serde(default)]
    pub pages: Option<String>,
}

#[async_trait]
impl TypedTool<ReadPdfParams> for ReadPdfTool {
    fn name(&self) -> &str {
        "ReadPdf"
    }

    fn description(&self) -> &str {
        "Reads and extracts text from a PDF file.\n\n\
         Usage:\n\
         - The file_path must be an absolute path to a .pdf file.\n\
         - For large PDFs (more than 10 pages), provide the pages parameter \
         to read specific page ranges (e.g., pages: \"1-5\"). Max 20 pages per request.\n\
         - Returns extracted text with page separators.\n\
         - PDFs containing only images will return a notice that no text was extractable."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: ReadPdfParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let file_path = &input.file_path;
        let pages = input.pages.filter(|s| !s.is_empty());

        let path = match ctx.backend.resolve_path(file_path, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !ext.eq_ignore_ascii_case("pdf") {
            return Ok(ToolResult::error(
                "ReadPdf only supports .pdf files. Use Read for other file types.",
            ));
        }

        match extract_pdf_text(path.as_path(), pages.as_deref()) {
            Ok(text) => Ok(ToolResult::success(text)),
            Err(e) => Ok(ToolResult::error(e)),
        }
    }
}

fn extract_pdf_text(path: &Path, pages: Option<&str>) -> Result<String, String> {
    let all_pages = pdf_extract::extract_text_by_pages(path)
        .map_err(|e| format!("Failed to extract PDF text: {e}"))?;

    if all_pages.is_empty() {
        return Ok("No extractable text (PDF may contain only images)".into());
    }

    let indices = match pages {
        Some(spec) => parse_page_range(spec, all_pages.len())?,
        None => (0..all_pages.len()).collect(),
    };

    let mut result = String::new();
    for &idx in &indices {
        let text = all_pages[idx].trim();
        result.push_str(&format!("--- Page {} ---\n", idx + 1));
        if text.is_empty() {
            result.push_str("(empty page)\n");
        } else {
            result.push_str(text);
            result.push('\n');
        }
        result.push('\n');
    }

    if result.trim().is_empty() {
        return Ok("No extractable text (PDF may contain only images)".into());
    }

    Ok(result)
}
