use std::sync::Arc;

use async_trait::async_trait;
use loopal_ipc::IpcBudget;
use loopal_output_guard::FinalSinkRedactionSeed;
use secrecy::SecretString;

use crate::error::SecretResult;
use crate::health::HubHealth;

#[async_trait]
pub trait SecretClient: Send + Sync {
    async fn get(&self, name: &str, budget: IpcBudget) -> SecretResult<SecretString>;

    async fn list_names(&self, budget: IpcBudget) -> SecretResult<Vec<String>>;

    async fn expand_author(&self, template: &str, budget: IpcBudget) -> SecretResult<SecretString>;

    async fn expand_wire(&self, template: &str, budget: IpcBudget) -> SecretResult<SecretString>;

    fn health(&self) -> Option<Arc<HubHealth>> {
        None
    }

    fn final_sink_redaction_seed(&self) -> Option<FinalSinkRedactionSeed> {
        None
    }
}
