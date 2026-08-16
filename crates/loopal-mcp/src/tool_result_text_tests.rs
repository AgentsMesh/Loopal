use loopal_protocol::McpContentBlock;
use rmcp::model::{CallToolResult, Content, RawResource, ResourceContents};

use super::{block_to_text, call_result_to_response};
use crate::BINARY_DENIED_MARKER;

#[test]
fn final_protocol_conversion_denies_unknown_binary_content() {
    let audio: Content = serde_json::from_value(serde_json::json!({
        "type": "audio", "data": "secret-payload", "mimeType": "audio/wav"
    }))
    .unwrap();
    let data_link = RawResource::new("data:image/png;base64,secret-payload", "image");
    let result = CallToolResult::success(vec![
        Content::image("secret-payload", "image/png"),
        audio,
        Content::resource(ResourceContents::blob("secret-payload", "file:///blob")),
        Content::embedded_text("data:text/plain,secret-payload", "secret-payload"),
        Content::resource_link(data_link),
    ]);

    let response = call_result_to_response(&result);
    assert_eq!(response.content.len(), 5);
    for block in &response.content {
        assert!(matches!(block, McpContentBlock::Text { text } if text == BINARY_DENIED_MARKER));
    }
    assert!(
        !serde_json::to_string(&response)
            .unwrap()
            .contains("secret-payload")
    );
}

#[test]
fn final_protocol_conversion_preserves_safe_text_and_links() {
    let link = RawResource::new("https://example.test/spec", "spec");
    let result = CallToolResult::error(vec![
        Content::text("plain"),
        Content::embedded_text("file:///safe.md", "body"),
        Content::resource_link(link),
    ]);

    let response = call_result_to_response(&result);
    assert!(response.is_error);
    assert!(matches!(&response.content[0], McpContentBlock::Text { text } if text == "plain"));
    assert_eq!(
        block_to_text(&response.content[1]),
        "[resource file:///safe.md]\nbody"
    );
    assert_eq!(
        block_to_text(&response.content[2]),
        "[resource: https://example.test/spec]"
    );
}

#[test]
fn proxy_projection_denies_typed_binary_and_embedded_uri_blocks() {
    for block in [
        McpContentBlock::Image {
            mime_type: "image/png".into(),
            data: "secret-payload".into(),
        },
        McpContentBlock::Audio {
            mime_type: "audio/wav".into(),
        },
        McpContentBlock::Resource {
            uri: "file:///opaque".into(),
            text: None,
        },
        McpContentBlock::ResourceLink {
            uri: " BLOB:https://example.test/id".into(),
        },
    ] {
        assert_eq!(block_to_text(&block), BINARY_DENIED_MARKER);
    }
}
