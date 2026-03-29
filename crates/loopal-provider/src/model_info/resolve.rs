use loopal_provider_api::ThinkingCapability;

use super::catalog::KNOWN_MODELS;
use super::overlay;

/// Resolve provider name from model id.
///
/// Checks the static catalog, then falls back to prefix heuristic.
/// User overlay is NOT checked here — `ProviderRegistry::resolve()` handles
/// that via `get_model_info()` before reaching this function.
pub fn resolve_provider(model_id: &str) -> &'static str {
    if let Some(entry) = KNOWN_MODELS.iter().find(|m| m.id == model_id) {
        return entry.provider;
    }
    resolve_provider_by_prefix(model_id)
}

/// Pure prefix heuristic — no catalog lookup.
pub fn resolve_provider_by_prefix(model_id: &str) -> &'static str {
    if model_id.starts_with("claude") {
        "anthropic"
    } else if model_id.starts_with("gpt-")
        || model_id.starts_with("o1")
        || model_id.starts_with("o3")
        || model_id.starts_with("o4")
    {
        "openai"
    } else if model_id.starts_with("gemini") {
        "google"
    } else {
        "openai_compat"
    }
}

/// Return the thinking capability for a model id.
/// Checks overlay → catalog → prefix heuristic.
pub fn get_thinking_capability(model_id: &str) -> ThinkingCapability {
    if let Some(info) = overlay::get_user_model(model_id) {
        return info.thinking;
    }
    if let Some(entry) = KNOWN_MODELS.iter().find(|m| m.id == model_id) {
        return entry.thinking;
    }
    if model_id.starts_with("o1") || model_id.starts_with("o3") || model_id.starts_with("o4") {
        ThinkingCapability::ReasoningEffort
    } else if model_id.contains("gemini-2.5") {
        ThinkingCapability::ThinkingBudget
    } else {
        ThinkingCapability::None
    }
}
