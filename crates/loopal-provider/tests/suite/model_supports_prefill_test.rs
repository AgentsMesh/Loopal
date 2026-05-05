use loopal_provider::get_model_info;

fn assert_supports_prefill(model_id: &str, expected: bool) {
    let info =
        get_model_info(model_id).unwrap_or_else(|| panic!("model {model_id} not in catalog"));
    assert_eq!(
        info.supports_prefill, expected,
        "supports_prefill mismatch for {model_id}: expected {expected}"
    );
}

#[test]
fn anthropic_haiku_supports_prefill() {
    // No built-in thinking → prefill allowed.
    assert_supports_prefill("claude-haiku-3-5-20241022", true);
}

#[test]
fn anthropic_thinking_models_forbid_prefill() {
    // BudgetRequired and Adaptive thinking models reject assistant tail
    // at the API level regardless of the `thinking` request field.
    // Misconfiguring any of these to `true` would re-introduce the bug.
    for model in [
        "claude-sonnet-4-20250514",
        "claude-opus-4-20250514",
        "claude-sonnet-4-6",
        "claude-opus-4-6",
        "claude-opus-4-7",
    ] {
        assert_supports_prefill(model, false);
    }
}

#[test]
fn openai_models_support_prefill() {
    for model in [
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4.1",
        "gpt-4.1-mini",
        "gpt-4.1-nano",
        "o3",
        "o3-mini",
        "o4-mini",
    ] {
        assert_supports_prefill(model, true);
    }
}

#[test]
fn google_models_support_prefill() {
    for model in [
        "gemini-2.0-flash",
        "gemini-2.5-pro-preview-05-06",
        "gemini-2.5-flash-preview-04-17",
    ] {
        assert_supports_prefill(model, true);
    }
}

#[test]
fn deepseek_models_support_prefill() {
    for model in ["deepseek-chat", "deepseek-reasoner"] {
        assert_supports_prefill(model, true);
    }
}
