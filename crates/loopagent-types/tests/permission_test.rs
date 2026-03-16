use loopagent_types::permission::{PermissionDecision, PermissionLevel, PermissionMode};

#[test]
fn test_default_allows_readonly() {
    assert_eq!(
        PermissionMode::Default.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
}

#[test]
fn test_default_asks_supervised() {
    assert_eq!(
        PermissionMode::Default.check(PermissionLevel::Supervised),
        PermissionDecision::Ask
    );
}

#[test]
fn test_default_asks_dangerous() {
    assert_eq!(
        PermissionMode::Default.check(PermissionLevel::Dangerous),
        PermissionDecision::Ask
    );
}

#[test]
fn test_accept_edits_allows_readonly_and_supervised() {
    assert_eq!(
        PermissionMode::AcceptEdits.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
    assert_eq!(
        PermissionMode::AcceptEdits.check(PermissionLevel::Supervised),
        PermissionDecision::Allow
    );
}

#[test]
fn test_accept_edits_asks_dangerous() {
    assert_eq!(
        PermissionMode::AcceptEdits.check(PermissionLevel::Dangerous),
        PermissionDecision::Ask
    );
}

#[test]
fn test_bypass_allows_all() {
    assert_eq!(
        PermissionMode::BypassPermissions.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
    assert_eq!(
        PermissionMode::BypassPermissions.check(PermissionLevel::Supervised),
        PermissionDecision::Allow
    );
    assert_eq!(
        PermissionMode::BypassPermissions.check(PermissionLevel::Dangerous),
        PermissionDecision::Allow
    );
}

#[test]
fn test_plan_allows_only_readonly() {
    assert_eq!(
        PermissionMode::Plan.check(PermissionLevel::ReadOnly),
        PermissionDecision::Allow
    );
    assert_eq!(
        PermissionMode::Plan.check(PermissionLevel::Supervised),
        PermissionDecision::Deny
    );
    assert_eq!(
        PermissionMode::Plan.check(PermissionLevel::Dangerous),
        PermissionDecision::Deny
    );
}
