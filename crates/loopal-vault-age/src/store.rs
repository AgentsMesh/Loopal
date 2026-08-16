use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_vault_api::{
    AuditMetadata, AuditSink, NoopAuditSink, Vault, VaultError, VaultOp, VaultResult,
};
use secrecy::SecretString;
use tokio::sync::RwLock;

use crate::identity::DiscoveredIdentity;
use crate::recipients::Recipients;
use crate::vault_io;

pub struct AgeVault {
    store_path: PathBuf,
    recipients_path: PathBuf,
    identity: Arc<DiscoveredIdentity>,
    audit: Arc<dyn AuditSink>,
    cache: RwLock<Option<BTreeMap<String, SecretString>>>,
}

impl AgeVault {
    pub fn new(
        store_path: PathBuf,
        recipients_path: PathBuf,
        identity: Arc<DiscoveredIdentity>,
    ) -> Self {
        Self::with_audit(
            store_path,
            recipients_path,
            identity,
            Arc::new(NoopAuditSink),
        )
    }

    pub fn with_audit(
        store_path: PathBuf,
        recipients_path: PathBuf,
        identity: Arc<DiscoveredIdentity>,
        audit: Arc<dyn AuditSink>,
    ) -> Self {
        Self {
            store_path,
            recipients_path,
            identity,
            audit,
            cache: RwLock::new(None),
        }
    }

    async fn load_if_needed(&self) -> VaultResult<()> {
        if self.cache.read().await.is_some() {
            return Ok(());
        }
        let mut write_guard = self.cache.write().await;
        if write_guard.is_some() {
            return Ok(());
        }
        self.identity.ensure_usable()?;
        *write_guard = Some(self.read_store_now()?);
        Ok(())
    }

    fn read_store_now(&self) -> VaultResult<BTreeMap<String, SecretString>> {
        let identity_dyn: &dyn age::Identity = &self.identity.identity;
        if self.store_path.exists() {
            vault_io::read_vault(&self.store_path, identity_dyn)
        } else {
            Ok(BTreeMap::new())
        }
    }

    async fn write_under_lock<F>(
        &self,
        op: VaultOp,
        key: &str,
        metadata: &AuditMetadata<'_>,
        mutator: F,
    ) -> VaultResult<()>
    where
        F: FnOnce(&mut BTreeMap<String, SecretString>),
    {
        let _lock = vault_io::acquire_store_lock(&self.store_path).await?;
        self.identity.ensure_usable()?;
        let recipients = Recipients::load(&self.recipients_path)?;
        if recipients.is_empty() {
            return Err(VaultError::EncryptionFailed(
                "no recipients configured".into(),
            ));
        }
        self.audit.record(op, key, metadata)?;
        let mut map = self.read_store_now()?;
        mutator(&mut map);
        vault_io::write_vault(&self.store_path, &recipients, &map)?;
        let mut guard = self.cache.write().await;
        *guard = Some(map);
        Ok(())
    }

    pub async fn get_audited(
        &self,
        name: &str,
        metadata: &AuditMetadata<'_>,
    ) -> VaultResult<Option<SecretString>> {
        self.audit.record(VaultOp::Decrypted, name, metadata)?;
        if let Some(value) = self
            .cache
            .read()
            .await
            .as_ref()
            .and_then(|map| map.get(name).cloned())
        {
            return Ok(Some(value));
        }
        if self.cache.read().await.is_some() {
            return Ok(None);
        }
        self.identity.ensure_usable()?;
        let mut write_guard = self.cache.write().await;
        if write_guard.is_none() {
            *write_guard = Some(self.read_store_now()?);
        }
        Ok(write_guard.as_ref().and_then(|map| map.get(name).cloned()))
    }
}

#[async_trait]
impl Vault for AgeVault {
    async fn get(&self, name: &str) -> Option<SecretString> {
        self.get_audited(name, &AuditMetadata::default())
            .await
            .ok()
            .flatten()
    }

    async fn list_names(&self) -> Vec<String> {
        if self.load_if_needed().await.is_err() {
            return Vec::new();
        }
        let guard = self.cache.read().await;
        guard
            .as_ref()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    async fn put(&self, name: &str, value: SecretString) -> VaultResult<()> {
        vault_io::validate_secret_name(name)?;
        let owned = name.to_string();
        self.write_under_lock(
            VaultOp::Encrypted,
            name,
            &AuditMetadata::default(),
            move |map| {
                map.insert(owned, value);
            },
        )
        .await
    }

    async fn delete(&self, name: &str) -> VaultResult<()> {
        let owned = name.to_string();
        self.write_under_lock(
            VaultOp::Encrypted,
            name,
            &AuditMetadata::default(),
            move |map| {
                map.remove(&owned);
            },
        )
        .await
    }

    async fn rekey(&self) -> VaultResult<()> {
        self.write_under_lock(VaultOp::Rekeyed, "", &AuditMetadata::default(), |_| {})
            .await
    }
}
