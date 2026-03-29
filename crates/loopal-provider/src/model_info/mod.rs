mod catalog;
pub mod overlay;
mod resolve;

use loopal_provider_api::ModelInfo;

use catalog::KNOWN_MODELS;

pub use overlay::init_user_models;
pub use resolve::{get_thinking_capability, resolve_provider, resolve_provider_by_prefix};

/// Return metadata for all known models (static catalog + user overrides).
/// User overrides replace matching catalog entries; new user models are appended.
pub fn list_all_models() -> Vec<ModelInfo> {
    let mut models: Vec<ModelInfo> = KNOWN_MODELS.iter().map(|m| m.to_model_info()).collect();
    if let Some(user_models) = overlay::all_user_models() {
        for (id, info) in user_models {
            if let Some(existing) = models.iter_mut().find(|m| m.id == *id) {
                *existing = info.clone();
            } else {
                models.push(info.clone());
            }
        }
    }
    models
}

/// Look up model metadata by id.
/// Priority: user overlay → static catalog → None.
pub fn get_model_info(model_id: &str) -> Option<ModelInfo> {
    if let Some(info) = overlay::get_user_model(model_id) {
        return Some(info);
    }
    KNOWN_MODELS
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.to_model_info())
}
