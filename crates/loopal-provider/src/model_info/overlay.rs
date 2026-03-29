use std::collections::HashMap;
use std::sync::OnceLock;

use loopal_provider_api::{
    CostTier, ModelInfo, ModelOverride, QualityTier, SpeedTier, ThinkingCapability,
};

use super::catalog::KNOWN_MODELS;

static USER_OVERRIDES: OnceLock<HashMap<String, ModelInfo>> = OnceLock::new();

/// Convert `settings.models` into resolved `ModelInfo` map and register them.
/// Each override is merged on top of the static catalog entry (if one exists)
/// or synthesized defaults.
pub fn init_user_models(raw: &HashMap<String, ModelOverride>) {
    let mut map = HashMap::with_capacity(raw.len());
    for (id, ov) in raw {
        let base = KNOWN_MODELS
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.to_model_info())
            .unwrap_or_else(|| synthesize(id, ov.provider.as_deref().unwrap_or("openai_compat")));
        let merged = apply_override(base, ov);
        map.insert(id.clone(), merged);
    }
    let _ = USER_OVERRIDES.set(map);
    // OnceLock::set silently fails on double init — acceptable since
    // bootstrap calls this exactly once. Subsequent calls (e.g. in tests
    // sharing a process) are harmless no-ops.
}

/// Look up a user-override model by id.
pub fn get_user_model(model_id: &str) -> Option<ModelInfo> {
    USER_OVERRIDES.get().and_then(|m| m.get(model_id)).cloned()
}

/// Return all user-override models (for list_all_models merging).
pub fn all_user_models() -> Option<&'static HashMap<String, ModelInfo>> {
    USER_OVERRIDES.get()
}

/// Synthesize ModelInfo for an unknown model using defaults.
pub fn synthesize(model_id: &str, provider: &str) -> ModelInfo {
    ModelInfo {
        id: model_id.to_string(),
        provider: provider.to_string(),
        display_name: model_id.to_string(),
        context_window: 128_000,
        max_output_tokens: 16_384,
        thinking: ThinkingCapability::None,
        speed: SpeedTier::Medium,
        cost: CostTier::Medium,
        quality: QualityTier::Standard,
        supports_tools: true,
        supports_vision: false,
    }
}

/// Apply a `ModelOverride` on top of a base `ModelInfo`.
/// Only fields present in the override (Some) are applied.
pub fn apply_override(mut base: ModelInfo, ov: &ModelOverride) -> ModelInfo {
    if let Some(ref p) = ov.provider {
        base.provider = p.clone();
    }
    if let Some(ref n) = ov.display_name {
        base.display_name = n.clone();
    }
    if let Some(v) = ov.context_window {
        base.context_window = v;
    }
    if let Some(v) = ov.max_output_tokens {
        base.max_output_tokens = v;
    }
    if let Some(v) = ov.speed {
        base.speed = v;
    }
    if let Some(v) = ov.cost {
        base.cost = v;
    }
    if let Some(v) = ov.quality {
        base.quality = v;
    }
    if let Some(v) = ov.supports_tools {
        base.supports_tools = v;
    }
    if let Some(v) = ov.supports_vision {
        base.supports_vision = v;
    }
    if let Some(v) = ov.thinking {
        base.thinking = v;
    }
    base
}
