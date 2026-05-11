use std::sync::Arc;

use loopal_error::LoopalError;

use crate::Provider;
use crate::model::TaskType;

pub trait ProviderResolver: Send + Sync {
    fn resolve_for(&self, task: TaskType) -> Result<(String, Arc<dyn Provider>), LoopalError>;
}
