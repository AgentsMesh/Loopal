use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use loopal_ipc::IpcBudget;
use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{
    SecretCaller, SecretGetResponse, SecretListNamesRequest, SecretListNamesResponse,
    WorkflowAttemptCapability, WorkflowPermissionCausation,
};
use secrecy::SecretString;

use crate::client::SecretClient;
use crate::error::{SecretError, SecretResult};
use crate::expand::expand_template;
use crate::health::HubHealth;
use crate::placeholder::{AUTHOR_RE, WIRE_RE};
use crate::retry::{RetryPolicy, classify_rpc, retry_transient};

#[path = "hub_client/authority.rs"]
mod authority;
use authority::SecretGetAuthority;

pub struct HubSecretClient {
    connection: Arc<Connection<Listening>>,
    cwd: PathBuf,
    agent_name: String,
    depth: u32,
    health: Arc<HubHealth>,
    retry_policy: RetryPolicy,
    final_sink_redaction_seed: FinalSinkRedactionSeed,
    get_authority: SecretGetAuthority,
}

impl HubSecretClient {
    pub fn new(
        connection: Arc<Connection<Listening>>,
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
            final_sink_redaction_seed: FinalSinkRedactionSeed::new(),
            get_authority: SecretGetAuthority::Agent,
        }
    }

    /// Build a startup-only workflow provider client; do not install it in the runtime Kernel.
    pub fn new_workflow_provider(
        connection: Arc<Connection<Listening>>,
        cwd: PathBuf,
        causation: WorkflowPermissionCausation,
        capability: WorkflowAttemptCapability,
    ) -> Self {
        Self {
            connection,
            cwd,
            agent_name: String::new(),
            depth: 0,
            health: Arc::new(HubHealth::new()),
            retry_policy: RetryPolicy::default(),
            final_sink_redaction_seed: FinalSinkRedactionSeed::new(),
            get_authority: SecretGetAuthority::WorkflowProvider {
                causation,
                capability,
            },
        }
    }

    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    pub fn with_final_sink_redaction_seed(mut self, seed: FinalSinkRedactionSeed) -> Self {
        self.final_sink_redaction_seed = seed;
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

    fn check_budget(budget: IpcBudget, op: &str) -> Result<Duration, SecretError> {
        match budget {
            IpcBudget::Forbidden => Err(SecretError::Ipc(format!(
                "{op} rejected: IpcBudget::Forbidden on critical path"
            ))),
            IpcBudget::Allowed(d) => Ok(d),
        }
    }

    fn get_request(&self, name: &str) -> Result<(&'static str, serde_json::Value), SecretError> {
        let caller = self.caller(None);
        self.get_authority.request(&self.cwd, name, caller)
    }
}

#[async_trait]
impl SecretClient for HubSecretClient {
    async fn get(&self, name: &str, budget: IpcBudget) -> SecretResult<SecretString> {
        let timeout = Self::check_budget(budget, "secret/get")?;
        let inner = retry_transient(self.retry_policy, || async {
            let (method, params) = self.get_request(name)?;
            let resp = self
                .connection
                .send_request(method, params)
                .await
                .map_err(|e| classify_rpc(&e))?;
            let payload: SecretGetResponse = serde_json::from_value(resp)
                .map_err(|e| SecretError::Ipc(format!("decode: {e}")))?;
            Ok(SecretString::from(payload.plaintext))
        });
        let result = match tokio::time::timeout(timeout, inner).await {
            Ok(r) => r,
            Err(_) => Err(SecretError::Ipc(format!(
                "secret/get timed out after {timeout:?}"
            ))),
        };
        let result = result.and_then(|value| {
            self.final_sink_redaction_seed
                .observe(name, value.clone())
                .map_err(|_| SecretError::Ipc("final-sink redaction seed unavailable".into()))?;
            Ok(value)
        });
        self.health.record_outcome(&result);
        result
    }

    async fn list_names(&self, budget: IpcBudget) -> SecretResult<Vec<String>> {
        let timeout = Self::check_budget(budget, "secret/list_names")?;
        let inner = retry_transient(self.retry_policy, || async {
            let req = SecretListNamesRequest {
                cwd: self.cwd.to_string_lossy().into_owned(),
            };
            let params =
                serde_json::to_value(&req).map_err(|e| SecretError::Ipc(format!("encode: {e}")))?;
            let resp = self
                .connection
                .send_request(methods::HUB_SECRET_LIST_NAMES.name, params)
                .await
                .map_err(|e| classify_rpc(&e))?;
            let payload: SecretListNamesResponse = serde_json::from_value(resp)
                .map_err(|e| SecretError::Ipc(format!("decode: {e}")))?;
            Ok(payload.names)
        });
        let result = match tokio::time::timeout(timeout, inner).await {
            Ok(r) => r,
            Err(_) => Err(SecretError::Ipc(format!(
                "secret/list_names timed out after {timeout:?}"
            ))),
        };
        self.health.record_outcome(&result);
        result
    }

    async fn expand_author(&self, template: &str, budget: IpcBudget) -> SecretResult<SecretString> {
        expand_template(&AUTHOR_RE, template, |n| async move {
            self.get(&n, budget).await
        })
        .await
    }

    async fn expand_wire(&self, template: &str, budget: IpcBudget) -> SecretResult<SecretString> {
        expand_template(
            &WIRE_RE,
            template,
            |n| async move { self.get(&n, budget).await },
        )
        .await
    }

    fn health(&self) -> Option<Arc<HubHealth>> {
        Some(self.health.clone())
    }

    fn final_sink_redaction_seed(&self) -> Option<FinalSinkRedactionSeed> {
        Some(self.final_sink_redaction_seed.clone())
    }
}

#[cfg(test)]
#[path = "hub_client/tests.rs"]
mod tests;
