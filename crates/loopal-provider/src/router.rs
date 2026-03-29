use std::collections::HashMap;
use std::sync::Arc;

use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::Provider;

use crate::model_info;

/// Registry that routes model names to the appropriate provider.
///
/// Resolution priority:
/// 1. Static catalog (known model → provider name)
/// 2. Prefix map (longest `model_prefix` match from openai_compat configs)
/// 3. Hardcoded prefix heuristic (claude → anthropic, gpt-/o* → openai, etc.)
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
    /// (prefix, provider_name) sorted by prefix length descending.
    prefix_map: Vec<(String, String)>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            prefix_map: Vec::new(),
        }
    }

    /// Register a provider by its name.
    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        self.providers.insert(provider.name().to_string(), provider);
    }

    /// Register a provider and associate a model prefix for routing.
    ///
    /// Must be called during single-threaded bootstrap only — the prefix map
    /// is re-sorted on each call and is read concurrently at runtime.
    pub fn register_with_prefix(&mut self, provider: Arc<dyn Provider>, prefix: &str) {
        let name = provider.name().to_string();
        self.providers.insert(name.clone(), provider);
        self.prefix_map.push((prefix.to_string(), name));
        // Sort by prefix length descending so longest prefix matches first.
        self.prefix_map.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    }

    /// Resolve which provider handles a given model ID.
    pub fn resolve(&self, model: &str) -> Result<Arc<dyn Provider>, LoopalError> {
        // 1. Check static catalog for an exact match.
        if let Some(info) = model_info::get_model_info(model) {
            if let Some(p) = self.providers.get(&info.provider) {
                return Ok(p.clone());
            }
        }
        // 2. Check user-configured prefix map (longest prefix wins).
        for (prefix, provider_name) in &self.prefix_map {
            if model.starts_with(prefix.as_str()) {
                if let Some(p) = self.providers.get(provider_name) {
                    return Ok(p.clone());
                }
            }
        }
        // 3. Hardcoded prefix heuristic.
        let provider_name = model_info::resolve_provider(model);
        self.providers.get(provider_name).cloned().ok_or_else(|| {
            LoopalError::Provider(ProviderError::ModelNotFound(format!(
                "no provider registered for '{model}' (resolved to '{provider_name}')"
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
