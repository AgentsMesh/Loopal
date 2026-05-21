use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use secrecy::SecretString;
use tokio::sync::RwLock;
use tracing::warn;

use loopal_secret_client::expand::expand_template;
use loopal_secret_client::placeholder::{AUTHOR_RE, WIRE_RE};
use loopal_secret_client::{SecretError, SecretResult};
use loopal_vault_age::{AgeVault, DiscoveredIdentity, discover, list_initialized_vaults};
use loopal_vault_api::{AuditSink, NoopAuditSink, Vault};

#[derive(Clone, Debug)]
pub struct AuditContext {
    pub agent_name: String,
    pub depth: u32,
    pub tool_name: Option<String>,
}

pub struct HubVaultService {
    vaults: RwLock<HashMap<PathBuf, Arc<AgeVault>>>,
    audit: Arc<dyn AuditSink>,
    identity: Arc<DiscoveredIdentity>,
}

impl HubVaultService {
    pub fn new(audit: Arc<dyn AuditSink>) -> SecretResult<Self> {
        let identity = discover()
            .map_err(|e| SecretError::DecryptFailed(format!("SSH identity: {e}")))?;
        Ok(Self::with_identity(Arc::new(identity), audit))
    }

    pub fn with_noop_audit() -> SecretResult<Self> {
        Self::new(Arc::new(NoopAuditSink))
    }

    /// Caller-supplied identity. Production uses `new()` to auto-discover
    /// from `~/.ssh/`; tests inject a known key, and non-`~/.ssh` deployments
    /// (e.g. secret manager) plug in here too.
    pub fn with_identity(
        identity: Arc<DiscoveredIdentity>,
        audit: Arc<dyn AuditSink>,
    ) -> Self {
        Self {
            vaults: RwLock::new(HashMap::new()),
            audit,
            identity,
        }
    }

    pub async fn get(
        &self,
        cwd: &Path,
        name: &str,
        _ctx: AuditContext,
    ) -> SecretResult<SecretString> {
        let vault = self.vault_for(cwd).await?;
        match vault.get(name).await {
            Some(s) => Ok(s),
            None => Err(SecretError::SecretNotFound(name.to_string())),
        }
    }

    pub async fn list_names(&self, cwd: &Path) -> SecretResult<Vec<String>> {
        let vault = self.vault_for(cwd).await?;
        Ok(vault.list_names().await)
    }

    pub async fn expand_author(
        &self,
        cwd: &Path,
        template: &str,
        ctx: AuditContext,
    ) -> SecretResult<SecretString> {
        let cwd_owned = cwd.to_path_buf();
        expand_template(&AUTHOR_RE, template, |name| {
            let cwd = cwd_owned.clone();
            let ctx = ctx.clone();
            async move { self.get(&cwd, &name, ctx).await }
        })
        .await
    }

    pub async fn expand_wire(
        &self,
        cwd: &Path,
        template: &str,
        ctx: AuditContext,
    ) -> SecretResult<SecretString> {
        let cwd_owned = cwd.to_path_buf();
        expand_template(&WIRE_RE, template, |name| {
            let cwd = cwd_owned.clone();
            let ctx = ctx.clone();
            async move { self.get(&cwd, &name, ctx).await }
        })
        .await
    }

    async fn vault_for(&self, cwd: &Path) -> SecretResult<Arc<AgeVault>> {
        let canonical = cwd
            .canonicalize()
            .map_err(|e| SecretError::VaultNotFound(cwd.join(format!("(canonicalize: {e})"))))?;
        if let Some(v) = self.vaults.read().await.get(&canonical) {
            return Ok(v.clone());
        }
        let mut write = self.vaults.write().await;
        if let Some(v) = write.get(&canonical) {
            return Ok(v.clone());
        }
        let arc = Arc::new(self.open_default_vault(&canonical)?);
        write.insert(canonical, arc.clone());
        Ok(arc)
    }

    fn open_default_vault(&self, cwd: &Path) -> SecretResult<AgeVault> {
        let vaults_dir = cwd.join(".loopal").join("vaults");
        let names = list_initialized_vaults(&vaults_dir);
        if names.is_empty() {
            return Err(SecretError::VaultNotFound(vaults_dir));
        }
        let name = if names.iter().any(|n| n == "default") {
            "default".to_string()
        } else {
            warn!(
                using = %names[0],
                "no 'default' vault present; using first alphabetical"
            );
            names[0].clone()
        };
        let store = vaults_dir.join(format!("{name}.vault")).join("store.age");
        let recipients = vaults_dir.join(format!("{name}.vault")).join("recipients");
        Ok(AgeVault::with_audit(
            store,
            recipients,
            self.identity.clone(),
            self.audit.clone(),
        ))
    }
}
