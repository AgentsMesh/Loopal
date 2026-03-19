use loopal_provider::{get_model_info, resolve_provider};

#[test]
fn test_get_known_model() {
    let info = get_model_info("claude-sonnet-4-20250514").unwrap();
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.context_window, 200_000);
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
    assert_eq!(info.max_output_tokens, 32_000);
    assert_eq!(info.input_price_per_mtok, 15.0);
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
