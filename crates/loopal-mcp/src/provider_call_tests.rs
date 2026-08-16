use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_config::{McpServerConfig, McpSharing};
use loopal_secret_client::{IpcBudget, SecretClient, SecretResult, SecretString};
use rmcp::model::RawContent;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

use super::client::McpClient;
use super::local_provider::LocalMcpProvider;
use super::manager::McpManager;
use super::provider::McpProvider;
use super::result_sanitizer::BINARY_DENIED_MARKER;

pub(crate) struct SeedClient;

#[async_trait]
impl SecretClient for SeedClient {
    async fn get(&self, _name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from("exact-plaintext"))
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn expand_author(
        &self,
        _template: &str,
        _budget: IpcBudget,
    ) -> SecretResult<SecretString> {
        unreachable!()
    }

    async fn expand_wire(&self, _template: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }
}

fn config(secret: bool) -> McpServerConfig {
    let env = secret.then(|| HashMap::from([("TOKEN".into(), "{{secret:token}}".into())]));
    McpServerConfig::Stdio {
        command: "mock".into(),
        args: Vec::new(),
        env: env.unwrap_or_default(),
        enabled: true,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

pub(crate) async fn fixture_client(
    capabilities: Value,
    responses: HashMap<String, Value>,
) -> McpClient {
    let (client, server) = tokio::io::duplex(8192);
    tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(server);
        let mut lines = BufReader::new(read).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let request: Value = serde_json::from_str(&line).unwrap();
            let Some(id) = request.get("id") else {
                continue;
            };
            let method = request["method"].as_str().unwrap_or_default();
            let result = match method {
                "initialize" => serde_json::json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": capabilities,
                    "serverInfo": {"name": "mock", "version": "1"}
                }),
                _ => responses.get(method).cloned().unwrap_or(Value::Null),
            };
            let reply = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
            write
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
        }
    });
    McpClient::connect(client, Duration::from_secs(5), None)
        .await
        .unwrap()
}

async fn mock_client(response: Value) -> McpClient {
    fixture_client(
        serde_json::json!({"tools": {}}),
        HashMap::from([
            (
                "tools/call".into(),
                serde_json::json!({
                    "content": [{"type": "text", "text": response.to_string()}]
                }),
            ),
            ("tools/list".into(), serde_json::json!({"tools": []})),
        ]),
    )
    .await
}

async fn provider_with_response(response: serde_json::Value, secret: bool) -> LocalMcpProvider {
    let client = mock_client(response).await;
    let mut manager = McpManager::new();
    if secret {
        manager.set_secret_client(Arc::new(SeedClient));
    }
    let configs = indexmap::IndexMap::from([("server".into(), config(secret))]);
    let mut prepared = manager.prepare_connections(&configs).await;
    let connection = prepared.pop().unwrap().with_client(client);
    manager.absorb_connections(vec![connection]).unwrap();
    LocalMcpProvider::new(Arc::new(RwLock::new(manager)))
}

#[tokio::test]
async fn exact_config_seed_redacts_server_echo() {
    let provider = provider_with_response(serde_json::json!("exact-plaintext"), true).await;
    let result = provider
        .call_tool(
            "server",
            "echo",
            &serde_json::json!({}),
            IpcBudget::Forbidden,
        )
        .await
        .unwrap();
    let encoded = serde_json::to_string(&result).unwrap();
    assert!(!encoded.contains("exact-plaintext"));
    assert!(encoded.contains("<secret_ref:token>"));
}

#[tokio::test]
async fn ordinary_nonsecret_response_remains_compatible() {
    let provider = provider_with_response(serde_json::json!("ordinary"), false).await;
    let result = provider
        .call_tool(
            "server",
            "echo",
            &serde_json::json!({}),
            IpcBudget::Forbidden,
        )
        .await
        .unwrap();
    assert!(matches!(
        &result.content[0].raw,
        RawContent::Text(text) if text.text.contains("ordinary")
    ));
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(BINARY_DENIED_MARKER)
    );
}
