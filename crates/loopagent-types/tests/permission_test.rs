use loopagent_types::permission::{PermissionDecision, PermissionLevel, PermissionMode};

#[test]
fn test_bypass_allows_all() {
    assert_eq!(
        PermissionMode::Bypass.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
    assert_eq!(
        PermissionMode::Bypass.check(PermissionLevel::Supervised),
        PermissionDecision::Allow
    );
    assert_eq!(
        PermissionMode::Bypass.check(PermissionLevel::Dangerous),
        PermissionDecision::Allow
    );
}

#[test]
fn test_supervised_allows_readonly() {
    assert_eq!(
        PermissionMode::Supervised.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
}

#[test]
fn test_supervised_asks_supervised() {
    assert_eq!(
        PermissionMode::Supervised.check(PermissionLevel::Supervised),
        PermissionDecision::Ask
    );
}

#[test]
fn test_supervised_asks_dangerous() {
    assert_eq!(
        PermissionMode::Supervised.check(PermissionLevel::Dangerous),
        PermissionDecision::Ask
    );
}
