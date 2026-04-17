use loopal_provider::{get_model_info, get_thinking_capability, list_all_models, resolve_provider};
use loopal_provider_api::{CostTier, QualityTier, SpeedTier, ThinkingCapability};

#[test]
fn test_get_known_model() {
    let info = get_model_info("claude-sonnet-4-20250514").unwrap();
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.context_window, 200_000);
    assert_eq!(info.max_output_tokens, 64_000);
}

#[test]
fn test_get_unknown_model() {
    assert!(get_model_info("nonexistent-model").is_none());
}

#[test]
fn test_get_openai_model() {
    let info = get_model_info("gpt-4o").unwrap();
    assert_eq!(info.provider, "openai");
    assert_eq!(info.display_name, "GPT-4o");
}

#[test]
fn test_get_google_model() {
    let info = get_model_info("gemini-2.0-flash").unwrap();
    assert_eq!(info.provider, "google");
    assert_eq!(info.display_name, "Gemini 2.0 Flash");
    assert_eq!(info.context_window, 1_000_000);
}

#[test]
fn test_get_opus_model() {
    let info = get_model_info("claude-opus-4-20250514").unwrap();
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.context_window, 200_000);
    assert_eq!(info.max_output_tokens, 32_000);
}

#[test]
fn test_get_opus_4_6_model() {
    let info = get_model_info("claude-opus-4-6").unwrap();
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.context_window, 1_000_000);
    assert_eq!(info.max_output_tokens, 128_000);
}

#[test]
fn test_get_sonnet_4_6_model() {
    let info = get_model_info("claude-sonnet-4-6").unwrap();
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.context_window, 1_000_000);
    assert_eq!(info.max_output_tokens, 64_000);
}

#[test]
fn test_get_opus_4_7_model() {
    let info = get_model_info("claude-opus-4-7").unwrap();
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.display_name, "Claude Opus 4.7");
    assert_eq!(info.context_window, 1_000_000);
    assert_eq!(info.max_output_tokens, 128_000);
    assert_eq!(info.thinking, ThinkingCapability::Adaptive);
}

#[test]
fn test_opus_4_7_resolves_to_anthropic() {
    assert_eq!(resolve_provider("claude-opus-4-7"), "anthropic");
}

#[test]
fn test_resolve_provider_anthropic() {
    assert_eq!(resolve_provider("claude-sonnet-4"), "anthropic");
}

#[test]
fn test_resolve_provider_openai() {
    assert_eq!(resolve_provider("gpt-4o"), "openai");
    assert_eq!(resolve_provider("o1-preview"), "openai");
    assert_eq!(resolve_provider("o3-mini"), "openai");
}

#[test]
fn test_resolve_provider_google() {
    assert_eq!(resolve_provider("gemini-2.0-flash"), "google");
}

#[test]
fn test_resolve_provider_unknown_fallback() {
    assert_eq!(resolve_provider("llama-3"), "openai_compat");
    assert_eq!(resolve_provider("mistral-7b"), "openai_compat");
}

// -- New model tier metadata -----------------------------------------------

#[test]
fn test_model_info_includes_tier_fields() {
    let info = get_model_info("claude-haiku-3-5-20241022").unwrap();
    assert_eq!(info.speed, SpeedTier::Fast);
    assert_eq!(info.cost, CostTier::Low);
    assert_eq!(info.quality, QualityTier::Basic);
    assert!(info.supports_tools);
    assert!(info.supports_vision);
}

#[test]
fn test_opus_is_premium_tier() {
    let info = get_model_info("claude-opus-4-6").unwrap();
    assert_eq!(info.speed, SpeedTier::Slow);
    assert_eq!(info.cost, CostTier::High);
    assert_eq!(info.quality, QualityTier::Premium);
}

#[test]
fn test_new_model_gpt_4_1() {
    let info = get_model_info("gpt-4.1").unwrap();
    assert_eq!(info.provider, "openai");
    assert_eq!(info.speed, SpeedTier::Medium);
    assert!(info.supports_tools);
}

#[test]
fn test_new_model_deepseek_chat() {
    let info = get_model_info("deepseek-chat").unwrap();
    assert_eq!(info.provider, "openai_compat");
    assert_eq!(info.cost, CostTier::Low);
    assert_eq!(info.display_name, "DeepSeek V3");
}

#[test]
fn test_new_model_o4_mini() {
    let info = get_model_info("o4-mini").unwrap();
    assert_eq!(info.provider, "openai");
    assert_eq!(info.thinking, ThinkingCapability::ReasoningEffort);
}

#[test]
fn test_new_model_gemini_2_5_flash() {
    let info = get_model_info("gemini-2.5-flash-preview-04-17").unwrap();
    assert_eq!(info.provider, "google");
    assert_eq!(info.speed, SpeedTier::Fast);
    assert_eq!(info.thinking, ThinkingCapability::ThinkingBudget);
}

// -- list_all_models -------------------------------------------------------

#[test]
fn test_list_all_models_has_at_least_18() {
    let models = list_all_models();
    assert!(
        models.len() >= 18,
        "expected >= 18 models, got {}",
        models.len()
    );
}

// -- get_thinking_capability -----------------------------------------------

#[test]
fn test_thinking_capability_known_model() {
    assert_eq!(
        get_thinking_capability("claude-sonnet-4-6"),
        ThinkingCapability::Adaptive
    );
    assert_eq!(
        get_thinking_capability("o3-mini"),
        ThinkingCapability::ReasoningEffort
    );
}

#[test]
fn test_thinking_capability_unknown_model_heuristic() {
    // o4-something → ReasoningEffort
    assert_eq!(
        get_thinking_capability("o4-turbo-xyz"),
        ThinkingCapability::ReasoningEffort
    );
    // gemini-2.5-xxx → ThinkingBudget
    assert_eq!(
        get_thinking_capability("gemini-2.5-custom"),
        ThinkingCapability::ThinkingBudget
    );
    // random unknown → None
    assert_eq!(
        get_thinking_capability("random-model"),
        ThinkingCapability::None
    );
}
