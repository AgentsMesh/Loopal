use loopal_mcp::{BINARY_DENIED_MARKER, HUB_RPC_BUDGET, McpProvider, McpProxyClient};
use rmcp::model::RawContent;
use serde_json::json;

use crate::proxy_client_support::MockHubClient;

#[tokio::test]
async fn untrusted_hub_binary_blocks_fail_closed_before_model_projection() {
    let mock = MockHubClient::new(vec![(
        "hub/mcp/call_tool",
        json!({
            "content": [
                {"type": "text", "text": "preamble"},
                {"type": "image", "mime_type": "image/png", "data": "secret-payload"},
                {"type": "audio", "mime_type": "audio/wav"},
                {"type": "resource", "uri": "file:///opaque", "text": null},
                {"type": "resource_link", "uri": "data:image/png;base64,secret-payload"},
                {"type": "resource_link", "uri": "BLOB:https://example.test/id"},
                {"type": "resource", "uri": "file:///safe.md", "text": "safe"},
                {"type": "resource_link", "uri": "https://example.test/spec"}
            ],
            "is_error": false
        }),
    )]);
    let result = McpProxyClient::new(mock)
        .call_tool("s", "t", &json!({}), HUB_RPC_BUDGET)
        .await
        .unwrap();
    let text = result
        .content
        .iter()
        .map(|content| match &content.raw {
            RawContent::Text(text) => text.text.as_str(),
            _ => panic!("proxy output was not text"),
        })
        .collect::<Vec<_>>();

    assert_eq!(text[0], "preamble");
    assert_eq!(text[1..6], [BINARY_DENIED_MARKER; 5]);
    assert_eq!(text[6], "[resource file:///safe.md]\nsafe");
    assert_eq!(text[7], "[resource: https://example.test/spec]");
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("secret-payload")
    );
}
