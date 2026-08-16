use loopal_protocol::{McpCallToolResponse, McpContentBlock};
use rmcp::model::{CallToolResult, RawContent, ResourceContents};

use crate::result_sanitizer::{BINARY_DENIED_MARKER, resource_uri_embeds_content};

pub fn call_result_to_response(result: &CallToolResult) -> McpCallToolResponse {
    let content = result.content.iter().map(content_to_block).collect();
    McpCallToolResponse {
        content,
        is_error: result.is_error.unwrap_or(false),
    }
}

pub fn block_to_text(block: &McpContentBlock) -> String {
    match block {
        McpContentBlock::Text { text } => text.clone(),
        McpContentBlock::Image { .. } | McpContentBlock::Audio { .. } => {
            BINARY_DENIED_MARKER.into()
        }
        McpContentBlock::Resource { uri, .. } if resource_uri_embeds_content(uri) => {
            BINARY_DENIED_MARKER.into()
        }
        McpContentBlock::Resource { uri, text } => match text {
            Some(text) => format!("[resource {uri}]\n{text}"),
            None => BINARY_DENIED_MARKER.into(),
        },
        McpContentBlock::ResourceLink { uri } if resource_uri_embeds_content(uri) => {
            BINARY_DENIED_MARKER.into()
        }
        McpContentBlock::ResourceLink { uri } => format!("[resource: {uri}]"),
    }
}

fn content_to_block(content: &rmcp::model::Content) -> McpContentBlock {
    match &content.raw {
        RawContent::Text(text) => McpContentBlock::Text {
            text: text.text.clone(),
        },
        RawContent::Image(_) | RawContent::Audio(_) => denied_block(),
        RawContent::Resource(resource) => match &resource.resource {
            ResourceContents::TextResourceContents { uri, text, .. }
                if !resource_uri_embeds_content(uri) =>
            {
                McpContentBlock::Resource {
                    uri: uri.clone(),
                    text: Some(text.clone()),
                }
            }
            _ => denied_block(),
        },
        RawContent::ResourceLink(link) if !resource_uri_embeds_content(&link.uri) => {
            McpContentBlock::ResourceLink {
                uri: link.uri.clone(),
            }
        }
        RawContent::ResourceLink(_) => denied_block(),
    }
}

fn denied_block() -> McpContentBlock {
    McpContentBlock::Text {
        text: BINARY_DENIED_MARKER.into(),
    }
}

#[cfg(test)]
#[path = "tool_result_text_tests.rs"]
mod tests;
