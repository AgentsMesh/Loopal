use std::collections::HashMap;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_error::McpError;
use loopal_tool_api::ToolDefinition;

use crate::connection::McpConnection;
use crate::manager::McpManager;
use crate::provider_call_tests::fixture_client;
use crate::result_sanitizer::BINARY_DENIED_MARKER;
use crate::types::{ConnectionStatus, McpPrompt, McpResource};

fn config() -> McpServerConfig {
    McpServerConfig::Stdio {
        command: "fixture".into(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: true,
        timeout_ms: 1,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

fn metadata_connection() -> McpConnection {
    let mut connection = McpConnection::new("server".into(), config(), None);
    connection.status = ConnectionStatus::Connected;
    connection.instructions = Some("Use carefully".into());
    connection.cached_tools = vec![ToolDefinition {
        name: "lookup".into(),
        description: "Lookup".into(),
        input_schema: serde_json::json!({"type": "object"}),
    }];
    connection.cached_resources = vec![McpResource {
        uri: "memory://one".into(),
        name: "one".into(),
        description: Some("First".into()),
        mime_type: Some("text/plain".into()),
    }];
    connection.cached_prompts = vec![McpPrompt {
        name: "summarize".into(),
        description: Some("Summarize".into()),
    }];
    connection
}

#[tokio::test]
async fn exposes_metadata_and_removes_tool_routes() {
    let mut manager = McpManager::new();
    manager
        .absorb_connections(vec![metadata_connection()])
        .unwrap();

    assert_eq!(manager.get_server_instructions()[0].1, "Use carefully");
    assert_eq!(manager.get_resources()[0].1.name, "one");
    assert_eq!(manager.get_prompts()[0].1.name, "summarize");
    assert_eq!(manager.get_tools_with_server()[0].1.name, "lookup");
    manager.remove_tool_mapping("lookup");
    assert!(
        manager
            .call_tool_by_name("lookup", &serde_json::json!({}))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn disconnect_returns_routes_and_clears_connection() {
    let mut manager = McpManager::new();
    manager
        .absorb_connections(vec![metadata_connection()])
        .unwrap();

    let removed = manager.disconnect_connection("server").await.unwrap();
    assert_eq!(removed, ["lookup"]);
    assert!(manager.get_tools_for_server("server").is_empty());
    assert!(manager.disconnect_connection("missing").await.is_err());
}

async fn resource_manager(contents: serde_json::Value, secret: bool) -> McpManager {
    let client = fixture_client(
        serde_json::json!({"resources": {}}),
        HashMap::from([
            (
                "resources/list".into(),
                serde_json::json!({"resources": []}),
            ),
            (
                "resources/read".into(),
                serde_json::json!({"contents": contents}),
            ),
        ]),
    )
    .await;
    let connection_config = if secret {
        let mut config = config();
        let McpServerConfig::Stdio { env, .. } = &mut config else {
            unreachable!()
        };
        env.insert("TOKEN".into(), "{{secret:token}}".into());
        config
    } else {
        config()
    };
    let connection = McpConnection::new("server".into(), connection_config, None)
        .with_secret_client(secret.then(|| {
            std::sync::Arc::new(crate::provider_call_tests::SeedClient)
                as std::sync::Arc<dyn loopal_secret_client::SecretClient>
        }))
        .with_client(client);
    let mut manager = McpManager::new();
    manager.absorb_connections(vec![connection]).unwrap();
    manager
}

#[tokio::test]
async fn reads_and_redacts_text_resources() {
    let manager = resource_manager(
        serde_json::json!([
            {"uri": "memory://a", "text": "first exact-plaintext"},
            {"uri": "memory://b", "text": "second"}
        ]),
        true,
    )
    .await;

    let text = manager
        .read_resource("server", "memory://all")
        .await
        .unwrap();
    assert_eq!(text, "first <secret_ref:token>\nsecond");
}

#[tokio::test]
async fn rejects_blob_resources_regardless_of_secret_seed() {
    for secret in [false, true] {
        let manager = resource_manager(
            serde_json::json!([{"uri": "memory://blob", "blob": "ZXhhY3Q="}]),
            secret,
        )
        .await;
        let error = manager
            .read_resource("server", "memory://blob")
            .await
            .unwrap_err();
        assert!(matches!(error, McpError::Protocol(message) if message == BINARY_DENIED_MARKER));
    }
}

#[tokio::test]
async fn snapshot_skips_stderr_while_the_tail_is_exclusively_borrowed() {
    let mut connection = McpConnection::new("server".into(), config(), None);
    connection.status = ConnectionStatus::Failed("failed".into());
    connection.errors.push("safe failure".into());
    let tail = connection.stderr_tail.clone();
    tail.lock().await.push_back("redacted marker".into());
    let mut manager = McpManager::new();
    assert!(manager.absorb_connections(vec![connection]).is_err());

    let _guard = tail.lock().await;
    let snapshots = manager.collect_snapshots();

    assert_eq!(snapshots[0].errors, ["safe failure"]);
}
