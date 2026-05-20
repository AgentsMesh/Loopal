use loopal_protocol::{McpCallToolResponse, McpContentBlock};
use rmcp::model::{CallToolResult, RawContent, ResourceContents};

/// Convert an rmcp `CallToolResult` into the IPC-friendly `McpCallToolResponse`
/// so hub forwarding preserves every content block (text, image, audio,
/// resource) across the protocol boundary.
pub fn call_result_to_response(result: &CallToolResult) -> McpCallToolResponse {
    let content = result.content.iter().filter_map(content_to_block).collect();
    McpCallToolResponse {
        content,
        is_error: result.is_error.unwrap_or(false),
    }
}

/// Render a single `McpContentBlock` as the text representation an LLM expects
/// when it cannot consume binary content directly. Mirrors the original
/// rmcp-native logic in `tool_adapter::content_to_text`.
pub fn block_to_text(block: &McpContentBlock) -> String {
    match block {
        McpContentBlock::Text { text } => text.clone(),
        McpContentBlock::Image { mime_type, data } => {
            format!("![image](data:{mime_type};base64,{data})")
        }
        McpContentBlock::Audio { mime_type } => format!("[audio: {mime_type}]"),
        McpContentBlock::Resource { uri, text } => match text {
            Some(t) => format!("[resource {uri}]\n{t}"),
            None => format!("[binary resource: {uri}]"),
        },
        McpContentBlock::ResourceLink { uri } => format!("[resource: {uri}]"),
    }
}

fn content_to_block(content: &rmcp::model::Content) -> Option<McpContentBlock> {
    match &content.raw {
        RawContent::Text(t) => Some(McpContentBlock::Text {
            text: t.text.clone(),
        }),
        RawContent::Image(img) => Some(McpContentBlock::Image {
            mime_type: img.mime_type.clone(),
            data: img.data.clone(),
        }),
        RawContent::Audio(audio) => Some(McpContentBlock::Audio {
            mime_type: audio.mime_type.clone(),
        }),
        RawContent::Resource(res) => Some(match &res.resource {
            ResourceContents::TextResourceContents { uri, text, .. } => McpContentBlock::Resource {
                uri: uri.clone(),
                text: Some(text.clone()),
            },
            ResourceContents::BlobResourceContents { uri, .. } => McpContentBlock::Resource {
                uri: uri.clone(),
                text: None,
            },
        }),
        RawContent::ResourceLink(link) => Some(McpContentBlock::ResourceLink {
            uri: link.uri.clone(),
        }),
    }
}
