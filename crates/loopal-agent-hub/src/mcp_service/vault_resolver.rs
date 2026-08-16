use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_hub_vault::{AuditContext, HubVaultService};
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_secret_client::{
    IpcBudget, SecretClient, SecretError, SecretResult, SecretString, expand_template,
};

pub(super) struct HubMcpSecretClient {
    vault: Arc<HubVaultService>,
    cwd: PathBuf,
    final_sink_redaction_seed: FinalSinkRedactionSeed,
}

impl HubMcpSecretClient {
    #[cfg(test)]
    pub(super) fn new(vault: Arc<HubVaultService>, cwd: PathBuf) -> Self {
        Self::new_with_redaction_seed(vault, cwd, FinalSinkRedactionSeed::new())
    }

    pub(super) fn new_with_redaction_seed(
        vault: Arc<HubVaultService>,
        cwd: PathBuf,
        final_sink_redaction_seed: FinalSinkRedactionSeed,
    ) -> Self {
        Self {
            vault,
            cwd,
            final_sink_redaction_seed,
        }
    }

    fn context() -> AuditContext {
        AuditContext {
            session_id: None,
            agent_name: "hub".into(),
            depth: 0,
            tool_name: Some("mcp/config".into()),
        }
    }
}

#[async_trait]
impl SecretClient for HubMcpSecretClient {
    async fn get(&self, name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        let value = self.vault.get(&self.cwd, name, Self::context()).await?;
        self.final_sink_redaction_seed
            .observe(name, value.clone())
            .map_err(|_| SecretError::Ipc("final-sink redaction seed unavailable".into()))?;
        Ok(value)
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        self.vault.list_names(&self.cwd).await
    }

    async fn expand_author(&self, template: &str, budget: IpcBudget) -> SecretResult<SecretString> {
        expand_template(
            &loopal_secret_client::placeholder::AUTHOR_RE,
            template,
            |name| async move { self.get(&name, budget).await },
        )
        .await
    }

    async fn expand_wire(&self, template: &str, budget: IpcBudget) -> SecretResult<SecretString> {
        expand_template(
            &loopal_secret_client::placeholder::WIRE_RE,
            template,
            |name| async move { self.get(&name, budget).await },
        )
        .await
    }

    fn final_sink_redaction_seed(&self) -> Option<FinalSinkRedactionSeed> {
        Some(self.final_sink_redaction_seed.clone())
    }
}

#[cfg(test)]
#[path = "vault_resolver_tests.rs"]
mod tests;
