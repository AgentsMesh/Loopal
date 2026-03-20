use std::path::Path;

/// Extract text from a PDF file, optionally filtering by page range.
///
/// Uses `pdf_extract::extract_text_by_pages` to get per-page text, then
/// formats each page as `--- Page N ---\n{text}\n`.
pub fn extract_pdf_text(path: &Path, pages: Option<&str>) -> Result<String, String> {
    let all_pages = pdf_extract::extract_text_by_pages(path)
        .map_err(|e| format!("Failed to extract PDF text: {}", e))?;

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

/// Parse a page range spec like `"1-5"`, `"3"`, `"10-20"` into 0-based indices.
///
/// Pages are 1-based in the spec but returned as 0-based indices.
pub fn parse_page_range(spec: &str, total: usize) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty page range".into());
    }

    if let Some((start_s, end_s)) = spec.split_once('-') {
        let start: usize = start_s.trim().parse()
            .map_err(|_| format!("invalid page number: '{}'", start_s.trim()))?;
        let end: usize = end_s.trim().parse()
            .map_err(|_| format!("invalid page number: '{}'", end_s.trim()))?;

        if start == 0 || end == 0 {
            return Err("page numbers are 1-based".into());
        }
        if start > end {
            return Err(format!("invalid range: start ({start}) > end ({end})"));
        }
        if start > total {
            return Err(format!("page {start} exceeds total pages ({total})"));
        }
        let end = end.min(total);
        Ok((start - 1..end).collect())
    } else {
        let page: usize = spec.parse()
            .map_err(|_| format!("invalid page number: '{spec}'"))?;
        if page == 0 {
            return Err("page numbers are 1-based".into());
        }
        if page > total {
            return Err(format!("page {page} exceeds total pages ({total})"));
        }
        Ok(vec![page - 1])
    }
}
