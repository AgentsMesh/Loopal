use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_config::{McpServerConfig, McpSharing};
use loopal_secret_client::{IpcBudget, SecretClient, SecretError, SecretResult, SecretString};

use crate::secret_provenance::SecretProvenance;

pub(crate) struct FakeClient {
    calls: AtomicUsize,
    failure: Option<SecretError>,
}

impl FakeClient {
    pub(crate) fn success() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            failure: None,
        })
    }

    pub(crate) fn failing(message: &str) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            failure: Some(SecretError::Ipc(message.into())),
        })
    }

    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SecretClient for FakeClient {
    async fn get(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.failure {
            Some(SecretError::Ipc(message)) => Err(SecretError::Ipc(message.clone())),
            Some(_) => Err(SecretError::PermissionDenied),
            None => Ok(SecretString::from("exact-plaintext")),
        }
    }

    async fn list_names(&self, _: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn expand_author(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }

    async fn expand_wire(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }
}

pub(crate) fn stdio(
    command: &str,
    args: Vec<&str>,
    env: HashMap<String, String>,
) -> McpServerConfig {
    McpServerConfig::Stdio {
        command: command.into(),
        args: args.into_iter().map(Into::into).collect(),
        env,
        enabled: true,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

pub(crate) fn client(fake: Arc<FakeClient>) -> Arc<dyn SecretClient> {
    fake
}

pub(crate) fn provenance() -> SecretProvenance {
    SecretProvenance::default()
}
