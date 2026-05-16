use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_secret_runtime::MergedVault;
use loopal_vault_api::{SecretString, Vault, VaultError, VaultResult};
use secrecy::ExposeSecret;
use tokio::sync::RwLock;

/// Minimal in-memory Vault for testing MergedVault routing.
struct MemVault {
    inner: RwLock<BTreeMap<String, String>>,
}

impl MemVault {
    fn with(pairs: &[(&str, &str)]) -> Arc<dyn Vault> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        Arc::new(Self {
            inner: RwLock::new(map),
        })
    }
}

#[async_trait]
impl Vault for MemVault {
    async fn get(&self, name: &str) -> Option<SecretString> {
        self.inner
            .read()
            .await
            .get(name)
            .map(|v| SecretString::from(v.clone()))
    }
    async fn list_names(&self) -> Vec<String> {
        self.inner.read().await.keys().cloned().collect()
    }
    async fn put(&self, name: &str, value: SecretString) -> VaultResult<()> {
        self.inner
            .write()
            .await
            .insert(name.to_string(), value.expose_secret().to_string());
        Ok(())
    }
    async fn delete(&self, name: &str) -> VaultResult<()> {
        if self.inner.write().await.remove(name).is_none() {
            return Err(VaultError::SecretNotFound(name.to_string()));
        }
        Ok(())
    }
    async fn rekey(&self) -> VaultResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn merged_list_names_unions_all_vaults_sorted() {
    let merged = MergedVault::new(
        (
            "default".to_string(),
            MemVault::with(&[("a", "1"), ("c", "3")]),
        ),
        vec![(
            "prod".to_string(),
            MemVault::with(&[("b", "2"), ("d", "4")]),
        )],
    );
    let names = merged.list_names().await;
    assert_eq!(names, vec!["a", "b", "c", "d"]);
}

#[tokio::test]
async fn merged_get_routes_to_owning_vault() {
    let merged = MergedVault::new(
        ("default".to_string(), MemVault::with(&[("a", "default_a")])),
        vec![("prod".to_string(), MemVault::with(&[("b", "prod_b")]))],
    );
    assert_eq!(merged.get("a").await.unwrap().expose_secret(), "default_a");
    assert_eq!(merged.get("b").await.unwrap().expose_secret(), "prod_b");
    assert!(merged.get("missing").await.is_none());
}

#[tokio::test]
async fn merged_conflict_default_wins() {
    let merged = MergedVault::new(
        (
            "default".to_string(),
            MemVault::with(&[("shared", "from_default")]),
        ),
        vec![(
            "prod".to_string(),
            MemVault::with(&[("shared", "from_prod")]),
        )],
    );
    let v = merged.get("shared").await.unwrap();
    assert_eq!(v.expose_secret(), "from_default");
}

#[tokio::test]
async fn merged_put_writes_default_and_invalidates_cache() {
    let merged = MergedVault::new(
        ("default".to_string(), MemVault::with(&[("a", "1")])),
        vec![("prod".to_string(), MemVault::with(&[("b", "2")]))],
    );
    // Prime cache.
    assert_eq!(merged.list_names().await, vec!["a", "b"]);
    // New key (not in any vault) → goes to default; cache invalidates.
    merged.put("new", SecretString::from("v")).await.unwrap();
    assert_eq!(merged.list_names().await, vec!["a", "b", "new"]);
    assert_eq!(merged.get("new").await.unwrap().expose_secret(), "v");
}

#[tokio::test]
async fn merged_put_existing_key_routes_to_owning_vault_not_default() {
    // Key "b" exists in "prod"; put through merged view must update prod's
    // copy, not create a default copy that would shadow it.
    let prod = MemVault::with(&[("b", "old_prod")]);
    let merged = MergedVault::new(
        ("default".to_string(), MemVault::with(&[])),
        vec![("prod".to_string(), prod.clone())],
    );
    let _ = merged.list_names().await; // prime cache → "b" → prod

    merged
        .put("b", SecretString::from("new_prod"))
        .await
        .unwrap();

    // Read via merged + via underlying prod vault: both see updated value,
    // and no duplicate exists in default.
    assert_eq!(merged.get("b").await.unwrap().expose_secret(), "new_prod");
    assert_eq!(prod.get("b").await.unwrap().expose_secret(), "new_prod");
    assert_eq!(merged.list_names().await, vec!["b"]);
}

#[tokio::test]
async fn merged_delete_routes_to_owning_vault() {
    let prod = MemVault::with(&[("b", "2")]);
    let merged = MergedVault::new(
        ("default".to_string(), MemVault::with(&[("a", "1")])),
        vec![("prod".to_string(), prod.clone())],
    );
    merged.delete("b").await.unwrap();
    assert!(prod.get("b").await.is_none());
    assert_eq!(merged.list_names().await, vec!["a"]);
}

#[tokio::test]
async fn merged_delete_missing_returns_secret_not_found() {
    let merged = MergedVault::new(("default".to_string(), MemVault::with(&[])), vec![]);
    let err = merged.delete("ghost").await.unwrap_err();
    assert!(matches!(err, VaultError::SecretNotFound(ref n) if n == "ghost"));
}

#[tokio::test]
async fn merged_single_vault_no_others_passes_through() {
    let merged = MergedVault::new(
        ("default".to_string(), MemVault::with(&[("only", "v")])),
        vec![],
    );
    assert_eq!(merged.list_names().await, vec!["only"]);
    assert_eq!(merged.get("only").await.unwrap().expose_secret(), "v");
}

#[tokio::test]
async fn merged_rekey_visits_all_vaults_and_invalidates() {
    let merged = MergedVault::new(
        ("default".to_string(), MemVault::with(&[("a", "1")])),
        vec![
            ("prod".to_string(), MemVault::with(&[("b", "2")])),
            ("staging".to_string(), MemVault::with(&[("c", "3")])),
        ],
    );
    let _ = merged.list_names().await; // prime
    merged.rekey().await.unwrap();
    assert_eq!(merged.list_names().await, vec!["a", "b", "c"]);
}
