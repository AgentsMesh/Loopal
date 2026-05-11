use loopal_tool_api::{PermissionDecision, PermissionLevel, PermissionMode};

#[test]
fn bypass_allows_all_levels() {
    for lvl in [
        PermissionLevel::ReadOnly,
        PermissionLevel::Write,
        PermissionLevel::Dangerous,
    ] {
        assert_eq!(
            PermissionMode::Bypass.check(lvl),
            PermissionDecision::Allow
        );
    }
}

#[test]
fn ask_dangerous_allows_readonly() {
    assert_eq!(
        PermissionMode::AskDangerous.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
}

#[test]
fn ask_dangerous_allows_supervised() {
    assert_eq!(
        PermissionMode::AskDangerous.check(PermissionLevel::Write),
        PermissionDecision::Allow
    );
}

#[test]
fn ask_dangerous_asks_dangerous() {
    assert_eq!(
        PermissionMode::AskDangerous.check(PermissionLevel::Dangerous),
        PermissionDecision::Ask
    );
}

#[test]
fn ask_any_write_allows_readonly() {
    assert_eq!(
        PermissionMode::AskAnyWrite.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
}

#[test]
fn ask_any_write_asks_supervised() {
    assert_eq!(
        PermissionMode::AskAnyWrite.check(PermissionLevel::Write),
        PermissionDecision::Ask
    );
}

#[test]
fn ask_any_write_asks_dangerous() {
    assert_eq!(
        PermissionMode::AskAnyWrite.check(PermissionLevel::Dangerous),
        PermissionDecision::Ask
    );
}

#[test]
fn serde_roundtrip_all_modes() {
    for mode in [
        PermissionMode::Bypass,
        PermissionMode::AskDangerous,
        PermissionMode::AskAnyWrite,
    ] {
        let json = serde_json::to_string(&mode).unwrap();
        let back: PermissionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, mode);
    }
}

#[test]
fn serde_uses_snake_case() {
    assert_eq!(
        serde_json::to_string(&PermissionMode::Bypass).unwrap(),
        "\"bypass\""
    );
    assert_eq!(
        serde_json::to_string(&PermissionMode::AskDangerous).unwrap(),
        "\"ask_dangerous\""
    );
    assert_eq!(
        serde_json::to_string(&PermissionMode::AskAnyWrite).unwrap(),
        "\"ask_any_write\""
    );
}

#[test]
fn copy_semantics_preserved() {
    let mode = PermissionMode::AskDangerous;
    let copy = mode;
    assert_eq!(mode, copy);
}

#[test]
fn from_str_accepts_all_modes() {
    assert_eq!(
        "bypass".parse::<PermissionMode>().unwrap(),
        PermissionMode::Bypass
    );
    assert_eq!(
        "ask_dangerous".parse::<PermissionMode>().unwrap(),
        PermissionMode::AskDangerous
    );
    assert_eq!(
        "ask_any_write".parse::<PermissionMode>().unwrap(),
        PermissionMode::AskAnyWrite
    );
}

#[test]
fn from_str_rejects_unknown_with_descriptive_error() {
    let err = "yolo".parse::<PermissionMode>().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("yolo"), "error must mention input: {msg}");
    assert!(
        msg.contains("ask_dangerous"),
        "error must list valid variants: {msg}"
    );
}

#[test]
fn display_round_trips_with_from_str() {
    for mode in [
        PermissionMode::Bypass,
        PermissionMode::AskDangerous,
        PermissionMode::AskAnyWrite,
    ] {
        assert_eq!(mode.to_string().parse::<PermissionMode>().unwrap(), mode);
    }
}
