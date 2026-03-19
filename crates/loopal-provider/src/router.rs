use std::collections::HashMap;
use std::sync::Arc;

use loopal_types::error::{LoopalError, ProviderError};
use loopal_types::provider::Provider;

use crate::model_info;

/// Registry that routes model names to the appropriate provider.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider by its name.
    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        self.providers
            .insert(provider.name().to_string(), provider);
    }

    /// Resolve which provider handles a given model ID.
    pub fn resolve(&self, model: &str) -> Result<Arc<dyn Provider>, LoopalError> {
        let provider_name = model_info::resolve_provider(model);
        self.providers
            .get(provider_name)
            .cloned()
            .ok_or_else(|| {
                LoopalError::Provider(ProviderError::ModelNotFound(format!(
                    "no provider registered for '{}' (resolved to '{}')",
                    model, provider_name
                )))
            })
    }

    /// Get a provider by its name directly.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
