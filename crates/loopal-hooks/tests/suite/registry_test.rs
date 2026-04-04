use loopal_config::{HookConfig, HookEvent};
use loopal_hooks::HookRegistry;

fn make_hook(event: HookEvent, tool_filter: Option<Vec<String>>) -> HookConfig {
    HookConfig {
        event,
        command: "echo test".into(),
        tool_filter,
        timeout_ms: 10_000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }
}

#[test]
fn test_match_by_event() {
    let reg = HookRegistry::new(vec![
        make_hook(HookEvent::PreToolUse, None),
        make_hook(HookEvent::PostToolUse, None),
    ]);
    let matched = reg.match_hooks(HookEvent::PreToolUse, None, None);
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].event, HookEvent::PreToolUse);
}

#[test]
fn test_match_with_tool_filter() {
    let reg = HookRegistry::new(vec![make_hook(
        HookEvent::PreToolUse,
        Some(vec!["bash".into(), "write".into()]),
    )]);
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("bash"), None)
            .len(),
        1
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("read"), None)
            .len(),
        0
    );
    assert_eq!(reg.match_hooks(HookEvent::PreToolUse, None, None).len(), 0);
}

#[test]
fn test_no_match_wrong_event() {
    let reg = HookRegistry::new(vec![make_hook(HookEvent::PreToolUse, None)]);
    assert!(
        reg.match_hooks(HookEvent::PostToolUse, None, None)
            .is_empty()
    );
}

#[test]
fn test_no_filter_matches_any_tool() {
    let reg = HookRegistry::new(vec![make_hook(HookEvent::PreToolUse, None)]);
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("anything"), None)
            .len(),
        1
    );
}

#[test]
fn test_match_hooks_returns_all_matching_for_event() {
    let reg = HookRegistry::new(vec![
        make_hook(HookEvent::PreToolUse, None),
        make_hook(HookEvent::PreToolUse, None),
        make_hook(HookEvent::PostToolUse, None),
    ]);
    let matched = reg.match_hooks(HookEvent::PreToolUse, None, None);
    assert_eq!(matched.len(), 2);
}

#[test]
fn test_match_hooks_with_tool_filter_only_matches_specified_tools() {
    let reg = HookRegistry::new(vec![
        make_hook(HookEvent::PreToolUse, Some(vec!["bash".into()])),
        make_hook(
            HookEvent::PreToolUse,
            Some(vec!["write".into(), "edit".into()]),
        ),
    ]);
    // "bash" matches only the first hook
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("bash"), None);
    assert_eq!(matched.len(), 1);

    // "write" matches only the second hook
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("write"), None);
    assert_eq!(matched.len(), 1);

    // "edit" matches only the second hook
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("edit"), None);
    assert_eq!(matched.len(), 1);

    // "unknown" matches neither
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("unknown"), None);
    assert_eq!(matched.len(), 0);
}

#[test]
fn test_empty_registry_returns_empty() {
    let reg = HookRegistry::new(vec![]);
    assert!(
        reg.match_hooks(HookEvent::PreToolUse, None, None)
            .is_empty()
    );
    assert!(
        reg.match_hooks(HookEvent::PreToolUse, Some("bash"), None)
            .is_empty()
    );
}

#[test]
fn test_mixed_filtered_and_unfiltered_hooks() {
    let reg = HookRegistry::new(vec![
        make_hook(HookEvent::PreToolUse, None), // matches any tool
        make_hook(HookEvent::PreToolUse, Some(vec!["bash".into()])), // only bash
    ]);

    // With tool_name "bash", both should match
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("bash"), None);
    assert_eq!(matched.len(), 2);

    // With tool_name "read", only the unfiltered one matches
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("read"), None);
    assert_eq!(matched.len(), 1);

    // With None tool_name, only the unfiltered one matches (filtered requires a tool name)
    let matched = reg.match_hooks(HookEvent::PreToolUse, None, None);
    assert_eq!(matched.len(), 1);
}

// ── Condition expression integration tests ──────────────────

fn make_condition_hook(event: HookEvent, condition: &str) -> HookConfig {
    HookConfig {
        event,
        command: "echo test".into(),
        tool_filter: None,
        timeout_ms: 10_000,
        condition: Some(condition.into()),
        id: None,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
    }
}

#[test]
fn test_condition_wildcard_matches_any_tool() {
    let reg = HookRegistry::new(vec![make_condition_hook(HookEvent::PreToolUse, "*")]);
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Bash"), None)
            .len(),
        1
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Write"), None)
            .len(),
        1
    );
}

#[test]
fn test_condition_exact_tool_name() {
    let reg = HookRegistry::new(vec![make_condition_hook(HookEvent::PreToolUse, "Bash")]);
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Bash"), None)
            .len(),
        1
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Write"), None)
            .len(),
        0
    );
}

#[test]
fn test_condition_or_syntax() {
    let reg = HookRegistry::new(vec![make_condition_hook(
        HookEvent::PreToolUse,
        "Bash|Write",
    )]);
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Bash"), None)
            .len(),
        1
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Write"), None)
            .len(),
        1
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Read"), None)
            .len(),
        0
    );
}

#[test]
fn test_condition_overrides_tool_filter() {
    let mut hook = make_condition_hook(HookEvent::PreToolUse, "Read");
    hook.tool_filter = Some(vec!["Bash".into()]);
    let reg = HookRegistry::new(vec![hook]);
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Bash"), None)
            .len(),
        0
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Read"), None)
            .len(),
        1
    );
}

#[test]
fn test_condition_glob_matches_with_tool_input() {
    let reg = HookRegistry::new(vec![make_condition_hook(
        HookEvent::PreToolUse,
        "Bash(git push*)",
    )]);
    let push = serde_json::json!({"command": "git push origin main"});
    let pull = serde_json::json!({"command": "git pull origin main"});
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Bash"), Some(&push))
            .len(),
        1
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Bash"), Some(&pull))
            .len(),
        0
    );
}

#[test]
fn test_condition_file_glob_matches_with_tool_input() {
    let reg = HookRegistry::new(vec![make_condition_hook(
        HookEvent::PreToolUse,
        "Write(*.rs)",
    )]);
    let rs = serde_json::json!({"file_path": "src/main.rs"});
    let ts = serde_json::json!({"file_path": "src/main.ts"});
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Write"), Some(&rs))
            .len(),
        1
    );
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("Write"), Some(&ts))
            .len(),
        0
    );
}
