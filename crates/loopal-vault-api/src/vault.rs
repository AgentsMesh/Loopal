use async_trait::async_trait;
use secrecy::SecretString;

use crate::error::VaultResult;

/// Encrypted key-value vault. Implementations may use age+yaml (default),
/// OS keychain, cloud KMS, or remote secret servers — downstream code
/// should depend on this trait, not concrete impls.
#[async_trait]
pub trait Vault: Send + Sync {
    async fn get(&self, name: &str) -> Option<SecretString>;
    async fn list_names(&self) -> Vec<String>;
    async fn put(&self, name: &str, value: SecretString) -> VaultResult<()>;
    async fn delete(&self, name: &str) -> VaultResult<()>;
    async fn rekey(&self) -> VaultResult<()>;
}
