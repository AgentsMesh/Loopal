use std::sync::Arc;

use async_trait::async_trait;
use secrecy::SecretString;

use crate::error::SecretResult;
use crate::health::HubHealth;

#[async_trait]
pub trait SecretClient: Send + Sync {
    async fn get(&self, name: &str) -> SecretResult<SecretString>;

    async fn list_names(&self) -> SecretResult<Vec<String>>;

    async fn expand_author(&self, template: &str) -> SecretResult<SecretString>;

    async fn expand_wire(&self, template: &str) -> SecretResult<SecretString>;

    /// Optional health tracker for transport-layer state. Default `None`
    /// for clients that don't track health (mocks, in-process). Returning
    /// `Some(HubHealth)` lets observers subscribe to degraded transitions
    /// and emit user-visible events.
    fn health(&self) -> Option<Arc<HubHealth>> {
        None
    }
}
