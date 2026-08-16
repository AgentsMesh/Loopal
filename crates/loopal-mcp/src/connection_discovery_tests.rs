use std::collections::HashMap;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_secret_client::SecretString;

use super::discover_capabilities;
use crate::connection::McpConnection;
use crate::provider_call_tests::fixture_client;
use crate::result_sanitizer::CallResultSanitizer;

fn config() -> McpServerConfig {
    McpServerConfig::Stdio {
        command: "fixture".into(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: true,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

#[tokio::test]
async fn discovers_all_advertised_capabilities() {
    let client = fixture_client(
        serde_json::json!({"tools": {}, "resources": {}, "prompts": {}}),
        HashMap::from([
            (
                "tools/list".into(),
                serde_json::json!({"tools": [{
                    "name": "lookup",
                    "description": "Lookup data",
                    "inputSchema": {"type": "object"}
                }]}),
            ),
            (
                "resources/list".into(),
                serde_json::json!({"resources": [{
                    "uri": "memory://item",
                    "name": "item",
                    "description": "Stored item",
                    "mimeType": "text/plain"
                }]}),
            ),
            (
                "prompts/list".into(),
                serde_json::json!({"prompts": [{
                    "name": "summarize",
                    "description": "Summarize an item"
                }]}),
            ),
        ]),
    )
    .await;
    let mut connection = McpConnection::new("fixture".into(), config(), None).with_client(client);

    discover_capabilities(&mut connection, &CallResultSanitizer::new(&[])).await;

    assert_eq!(connection.cached_tools[0].name, "lookup");
    assert_eq!(connection.cached_tools[0].description, "Lookup data");
    assert_eq!(connection.cached_tools[0].input_schema["type"], "object");
    assert_eq!(connection.cached_resources[0].uri, "memory://item");
    assert_eq!(connection.cached_resources[0].name, "item");
    assert_eq!(
        connection.cached_resources[0].description.as_deref(),
        Some("Stored item")
    );
    assert_eq!(
        connection.cached_resources[0].mime_type.as_deref(),
        Some("text/plain")
    );
    assert_eq!(connection.cached_prompts[0].name, "summarize");
    assert_eq!(
        connection.cached_prompts[0].description.as_deref(),
        Some("Summarize an item")
    );
    assert!(connection.errors.is_empty());
}

#[tokio::test]
async fn redacts_all_discovery_metadata_before_caching() {
    let client = fixture_client(
        serde_json::json!({"tools": {}, "resources": {}, "prompts": {}}),
        HashMap::from([
            (
                "tools/list".into(),
                serde_json::json!({"tools": [{
                    "name": "tool-exact-plaintext",
                    "description": "description exact-plaintext",
                    "inputSchema": {
                        "exact-plaintext": "exact-plaintext",
                        "description": "schema exact-plaintext"
                    }
                }]}),
            ),
            (
                "resources/list".into(),
                serde_json::json!({"resources": [{
                    "uri": "memory://exact-plaintext",
                    "name": "resource-exact-plaintext",
                    "description": "resource exact-plaintext",
                    "mimeType": "type/exact-plaintext"
                }]}),
            ),
            (
                "prompts/list".into(),
                serde_json::json!({"prompts": [{
                    "name": "prompt-exact-plaintext",
                    "description": "prompt exact-plaintext"
                }]}),
            ),
        ]),
    )
    .await;
    let mut connection = McpConnection::new("fixture".into(), config(), None).with_client(client);
    let sanitizer =
        CallResultSanitizer::new(&[("token".into(), SecretString::from("exact-plaintext"))]);

    discover_capabilities(&mut connection, &sanitizer).await;

    let encoded = format!(
        "{:?}{:?}{:?}",
        connection.cached_tools, connection.cached_resources, connection.cached_prompts
    );
    assert!(!encoded.contains("exact-plaintext"));
    assert!(encoded.contains("<secret_ref:token>"));
}

#[tokio::test]
async fn skips_capabilities_the_server_does_not_advertise() {
    let client = fixture_client(serde_json::json!({}), HashMap::new()).await;
    let mut connection = McpConnection::new("fixture".into(), config(), None).with_client(client);

    discover_capabilities(&mut connection, &CallResultSanitizer::new(&[])).await;

    assert!(connection.cached_tools.is_empty());
    assert!(connection.cached_resources.is_empty());
    assert!(connection.cached_prompts.is_empty());
    assert!(connection.errors.is_empty());
}

#[tokio::test]
async fn preserves_advertised_items_without_optional_metadata() {
    let client = fixture_client(
        serde_json::json!({"tools": {}, "resources": {}, "prompts": {}}),
        HashMap::from([
            (
                "tools/list".into(),
                serde_json::json!({"tools": [{
                    "name": "lookup", "inputSchema": {"type": "object"}
                }]}),
            ),
            (
                "resources/list".into(),
                serde_json::json!({"resources": [{
                    "uri": "memory://item", "name": "item"
                }]}),
            ),
            (
                "prompts/list".into(),
                serde_json::json!({"prompts": [{"name": "summarize"}]}),
            ),
        ]),
    )
    .await;
    let mut connection = McpConnection::new("fixture".into(), config(), None).with_client(client);

    discover_capabilities(&mut connection, &CallResultSanitizer::new(&[])).await;

    assert!(connection.cached_tools[0].description.is_empty());
    assert!(connection.cached_resources[0].description.is_none());
    assert!(connection.cached_resources[0].mime_type.is_none());
    assert!(connection.cached_prompts[0].description.is_none());
}
