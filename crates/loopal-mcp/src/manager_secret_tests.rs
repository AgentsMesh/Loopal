use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use loopal_config::{McpServerConfig, McpSharing};
use loopal_secret_client::{IpcBudget, SecretClient, SecretError, SecretResult, SecretString};

use super::manager::McpManager;
use super::secret_expand::CONFIG_SECRET_ERROR;

struct FailingClient;

#[async_trait]
impl SecretClient for FailingClient {
    async fn get(&self, _name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Err(SecretError::Ipc("audit failed with plaintext".into()))
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

fn stdio(env: HashMap<String, String>, enabled: bool) -> McpServerConfig {
    McpServerConfig::Stdio {
        command: "/usr/bin/true".into(),
        args: Vec::new(),
        env,
        enabled,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

#[tokio::test]
async fn config_failure_isolated_to_server_and_snapshot_is_safe() {
    let mut manager = McpManager::new();
    manager.set_secret_client(Arc::new(FailingClient));
    let configs = IndexMap::from([
        (
            "failed".into(),
            stdio(
                HashMap::from([("TOKEN".into(), "{{secret:private_name}}".into())]),
                true,
            ),
        ),
        ("disabled".into(), stdio(HashMap::new(), false)),
        ("ordinary".into(), stdio(HashMap::new(), true)),
    ]);
    let mut prepared = manager.prepare_connections(&configs).await;
    assert_eq!(prepared.len(), 2);
    assert!(
        prepared
            .iter()
            .any(|connection| connection.name == "ordinary")
    );
    let failed = prepared
        .iter_mut()
        .find(|connection| connection.name == "failed")
        .unwrap();
    assert!(!failed.status.is_failed());
    failed.connect().await;
    assert!(failed.status.is_failed());
    assert_eq!(failed.errors, [CONFIG_SECRET_ERROR]);

    let _ = manager.absorb_connections(prepared);
    let snapshot = manager
        .collect_snapshots()
        .into_iter()
        .find(|snapshot| snapshot.name == "failed")
        .unwrap();
    let encoded = format!("{} {:?}", snapshot.status, snapshot.errors);
    assert!(encoded.contains(CONFIG_SECRET_ERROR));
    assert!(!encoded.contains("private_name"));
    assert!(!encoded.contains("plaintext"));
}
