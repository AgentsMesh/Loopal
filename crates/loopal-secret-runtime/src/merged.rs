use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_vault_api::{SecretString, Vault, VaultError, VaultResult};
use tokio::sync::RwLock;
use tracing::warn;

/// Composes multiple named vaults into a single `Vault` view.
///
/// `default` is consulted first; conflicting names across other vaults are
/// logged and shadowed in favour of `default` (then by alphabetical order
/// of the other vaults).
///
/// LLM-bound code only sees the merged view (flat `<secret_ref:NAME>`); the
/// vault each name resolves to is internal.
///
/// The name→vault index is rebuilt lazily and invalidated after any
/// mutating operation, so a `put` followed by a `list_names` sees the new
/// key. Cross-process changes are NOT detected; cache is per-process state.
pub struct MergedVault {
    default: (String, Arc<dyn Vault>),
    others: Vec<(String, Arc<dyn Vault>)>,
    name_to_vault: RwLock<Option<HashMap<String, String>>>,
}

impl MergedVault {
    /// Build a merged view. `default` is the default-priority vault; `others`
    /// is sorted by caller (typically alphabetical).
    pub fn new(default: (String, Arc<dyn Vault>), others: Vec<(String, Arc<dyn Vault>)>) -> Self {
        Self {
            default,
            others,
            name_to_vault: RwLock::new(None),
        }
    }

    async fn resolve_map(&self) -> HashMap<String, String> {
        if let Some(map) = self.name_to_vault.read().await.as_ref() {
            return map.clone();
        }
        let mut guard = self.name_to_vault.write().await;
        if let Some(map) = guard.as_ref() {
            return map.clone();
        }
        let fresh = self.build_map().await;
        *guard = Some(fresh.clone());
        fresh
    }

    async fn invalidate(&self) {
        *self.name_to_vault.write().await = None;
    }

    async fn build_map(&self) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::new();
        let mut conflicts: Vec<(String, String, String)> = Vec::new();

        let (default_name, default_vault) = &self.default;
        for n in default_vault.list_names().await {
            map.insert(n, default_name.clone());
        }
        for (other_name, other_vault) in &self.others {
            for n in other_vault.list_names().await {
                if let Some(winner) = map.get(&n) {
                    conflicts.push((n.clone(), winner.clone(), other_name.clone()));
                } else {
                    map.insert(n, other_name.clone());
                }
            }
        }
        for (key, winner, shadowed) in &conflicts {
            warn!(
                key = key.as_str(),
                winner = winner.as_str(),
                shadowed = shadowed.as_str(),
                "secret name conflict between vaults; using winner"
            );
        }
        map
    }

    fn vault_by_name(&self, name: &str) -> Option<&Arc<dyn Vault>> {
        if name == self.default.0 {
            return Some(&self.default.1);
        }
        self.others.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

#[async_trait]
impl Vault for MergedVault {
    async fn get(&self, name: &str) -> Option<SecretString> {
        let map = self.resolve_map().await;
        let vault_name = map.get(name)?;
        let vault = self.vault_by_name(vault_name)?;
        vault.get(name).await
    }

    async fn list_names(&self) -> Vec<String> {
        let map = self.resolve_map().await;
        let mut names: Vec<String> = map.keys().cloned().collect();
        names.sort();
        names
    }

    async fn put(&self, name: &str, value: SecretString) -> VaultResult<()> {
        // Route to the vault that currently owns this name; falls back to
        // default for new names. Avoids creating duplicate copies that would
        // then trigger conflict warnings on next list.
        let map = self.resolve_map().await;
        let target_vault = match map.get(name) {
            Some(owner_name) => self
                .vault_by_name(owner_name)
                .ok_or_else(|| {
                    VaultError::Backend(format!("internal: vault {owner_name} not found"))
                })?
                .clone(),
            None => self.default.1.clone(),
        };
        target_vault.put(name, value).await?;
        self.invalidate().await;
        Ok(())
    }

    async fn delete(&self, name: &str) -> VaultResult<()> {
        let map = self.resolve_map().await;
        let Some(vault_name) = map.get(name).cloned() else {
            return Err(VaultError::SecretNotFound(name.to_string()));
        };
        let Some(vault) = self.vault_by_name(&vault_name) else {
            return Err(VaultError::Backend(format!(
                "internal: vault {vault_name} not found"
            )));
        };
        vault.delete(name).await?;
        self.invalidate().await;
        Ok(())
    }

    async fn rekey(&self) -> VaultResult<()> {
        self.default.1.rekey().await?;
        for (_, v) in &self.others {
            v.rekey().await?;
        }
        self.invalidate().await;
        Ok(())
    }
}
