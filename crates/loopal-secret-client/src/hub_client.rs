use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use secrecy::SecretString;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    SecretCaller, SecretGetRequest, SecretGetResponse, SecretListNamesRequest,
    SecretListNamesResponse,
};

use crate::client::SecretClient;
use crate::error::{SecretError, SecretResult};
use crate::expand::expand_template;
use crate::health::HubHealth;
use crate::placeholder::{AUTHOR_RE, WIRE_RE};
use crate::retry::{RetryPolicy, classify_rpc, retry_transient};

pub struct HubSecretClient {
    connection: Arc<Connection>,
    cwd: PathBuf,
    agent_name: String,
    depth: u32,
    health: Arc<HubHealth>,
    retry_policy: RetryPolicy,
}

impl HubSecretClient {
    pub fn new(
        connection: Arc<Connection>,
        cwd: PathBuf,
        agent_name: String,
        depth: u32,
    ) -> Self {
        Self {
            connection,
            cwd,
            agent_name,
            depth,
            health: Arc::new(HubHealth::new()),
            retry_policy: RetryPolicy::default(),
        }
    }

    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    pub fn health(&self) -> Arc<HubHealth> {
        self.health.clone()
    }

    fn caller(&self, tool_name: Option<String>) -> SecretCaller {
        SecretCaller {
            agent_name: self.agent_name.clone(),
            depth: self.depth,
            tool_name,
        }
    }
}

#[async_trait]
impl SecretClient for HubSecretClient {
    async fn get(&self, name: &str) -> SecretResult<SecretString> {
        let result = retry_transient(self.retry_policy, || async {
            let req = SecretGetRequest {
                cwd: self.cwd.to_string_lossy().into_owned(),
                name: name.to_string(),
                caller: self.caller(None),
            };
            let params = serde_json::to_value(&req)
                .map_err(|e| SecretError::Ipc(format!("encode: {e}")))?;
            let resp = self
                .connection
                .send_request(methods::HUB_SECRET_GET.name, params)
                .await
                .map_err(|e| classify_rpc(&e))?;
            let payload: SecretGetResponse = serde_json::from_value(resp)
                .map_err(|e| SecretError::Ipc(format!("decode: {e}")))?;
            Ok(SecretString::from(payload.plaintext))
        })
        .await;
        self.health.record_outcome(&result);
        result
    }

    async fn list_names(&self) -> SecretResult<Vec<String>> {
        let result = retry_transient(self.retry_policy, || async {
            let req = SecretListNamesRequest {
                cwd: self.cwd.to_string_lossy().into_owned(),
            };
            let params = serde_json::to_value(&req)
                .map_err(|e| SecretError::Ipc(format!("encode: {e}")))?;
            let resp = self
                .connection
                .send_request(methods::HUB_SECRET_LIST_NAMES.name, params)
                .await
                .map_err(|e| classify_rpc(&e))?;
            let payload: SecretListNamesResponse = serde_json::from_value(resp)
                .map_err(|e| SecretError::Ipc(format!("decode: {e}")))?;
            Ok(payload.names)
        })
        .await;
        self.health.record_outcome(&result);
        result
    }

    async fn expand_author(&self, template: &str) -> SecretResult<SecretString> {
        expand_template(&AUTHOR_RE, template, |n| async move { self.get(&n).await }).await
    }

    async fn expand_wire(&self, template: &str) -> SecretResult<SecretString> {
        expand_template(&WIRE_RE, template, |n| async move { self.get(&n).await }).await
    }

    fn health(&self) -> Option<Arc<HubHealth>> {
        Some(self.health.clone())
    }
}
