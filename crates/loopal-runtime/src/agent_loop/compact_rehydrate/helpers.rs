use loopal_provider_api::ContentBlock;
use loopal_tool_api::ToolResult;
use loopal_turn::{RehydratedFile, ToolCallId};

pub(super) fn collect_rehydrated_files(
    use_blocks: &[ContentBlock],
    result_blocks: &[ContentBlock],
) -> Vec<RehydratedFile> {
    let mut results: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for b in result_blocks {
        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } = b
        {
            results.insert(tool_use_id.as_str(), content.as_str());
        }
    }
    use_blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, input, .. } => {
                let body = results.get(id.as_str())?;
                let path = input
                    .get("file_path")
                    .or_else(|| input.get("path"))
                    .and_then(|v| v.as_str())?;
                Some(RehydratedFile {
                    path: path.to_string(),
                    tool_call_id: ToolCallId::new(id),
                    content: body.to_string(),
                })
            }
            _ => None,
        })
        .collect()
}

pub(super) fn trim_body(r: ToolResult, max_bytes: usize) -> String {
    if r.content.len() <= max_bytes {
        return r.content;
    }
    let mut end = max_bytes;
    while end > 0 && !r.content.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[...{} bytes truncated]",
        &r.content[..end],
        r.content.len() - end
    )
}
