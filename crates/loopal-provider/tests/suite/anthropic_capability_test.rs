use loopal_provider::anthropic::capability::supports_temperature;

#[test]
fn opus_4_family_rejected() {
    assert!(!supports_temperature("claude-opus-4-7"));
    assert!(!supports_temperature("claude-opus-4-1"));
    assert!(!supports_temperature("claude-opus-4-20250514"));
}

#[test]
fn sonnet_4_family_rejected() {
    assert!(!supports_temperature("claude-sonnet-4-6"));
    assert!(!supports_temperature("claude-sonnet-4-5"));
    assert!(!supports_temperature("claude-sonnet-4-20250514"));
}

#[test]
fn haiku_4_family_rejected() {
    assert!(!supports_temperature("claude-haiku-4-5"));
    assert!(!supports_temperature("claude-haiku-4-5-20251001"));
}

#[test]
fn claude_3_family_accepted() {
    assert!(supports_temperature("claude-3-opus-20240229"));
    assert!(supports_temperature("claude-3-5-sonnet-20241022"));
    assert!(supports_temperature("claude-3-5-haiku-20241022"));
    assert!(supports_temperature("claude-3-7-sonnet-latest"));
}

#[test]
fn claude_2_family_accepted() {
    assert!(supports_temperature("claude-2"));
    assert!(supports_temperature("claude-2.1"));
}

#[test]
fn instant_accepted() {
    assert!(supports_temperature("claude-instant-1.2"));
}

#[test]
fn case_insensitive_matching() {
    assert!(supports_temperature("Claude-3-Opus"));
    assert!(supports_temperature("CLAUDE-3-5-SONNET"));
    assert!(!supports_temperature("Claude-Opus-4-7"));
}

#[test]
fn unknown_future_models_default_to_rejected() {
    // Conservative default: any model not explicitly on the allowlist
    // is presumed to NOT accept temperature.
    assert!(!supports_temperature("claude-5-opus"));
    assert!(!supports_temperature("claude-next"));
    assert!(!supports_temperature("some-future-model"));
    assert!(!supports_temperature(""));
}
