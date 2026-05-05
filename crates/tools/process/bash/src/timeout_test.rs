use super::*;

#[test]
fn timeout_secs_converts_to_duration() {
    let t = TimeoutSecs::from_tool_input(&json!({"timeout": 120}), 0);
    assert_eq!(t.as_secs(), 120);
    assert_eq!(t.to_duration_clamped(MAX_TIMEOUT), Duration::from_secs(120));
}

#[test]
fn timeout_secs_clamps_to_max() {
    let t = TimeoutSecs::from_tool_input(&json!({"timeout": 700}), 0);
    assert_eq!(t.to_duration_clamped(MAX_TIMEOUT), MAX_TIMEOUT);
}

#[test]
fn timeout_secs_uses_default_when_missing() {
    let t = TimeoutSecs::from_tool_input(&json!({}), DEFAULT_TIMEOUT_SECS);
    assert_eq!(t.as_secs(), DEFAULT_TIMEOUT_SECS);
    let t2 = TimeoutSecs::from_tool_input(&json!({"command": "ls"}), 42);
    assert_eq!(t2.as_secs(), 42);
}

#[test]
fn timeout_secs_zero_yields_zero() {
    let t = TimeoutSecs::from_tool_input(&json!({"timeout": 0}), DEFAULT_TIMEOUT_SECS);
    assert_eq!(t.as_secs(), 0);
    assert_eq!(t.to_duration_clamped(MAX_TIMEOUT), Duration::ZERO);
}

#[test]
fn timeout_secs_display() {
    assert_eq!(TimeoutSecs::new(300).to_string(), "300s");
    assert_eq!(TimeoutSecs::new(0).to_string(), "0s");
}
