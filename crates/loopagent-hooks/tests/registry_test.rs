use loopagent_hooks::HookRegistry;
use loopagent_types::hook::{HookConfig, HookEvent};

fn make_hook(event: HookEvent, tool_filter: Option<Vec<String>>) -> HookConfig {
    HookConfig {
        event,
        command: "echo test".into(),
        tool_filter,
        timeout_ms: 10_000,
    }
}

#[test]
fn test_match_by_event() {
    let reg = HookRegistry::new(vec![
        make_hook(HookEvent::PreToolUse, None),
        make_hook(HookEvent::PostToolUse, None),
    ]);
    let matched = reg.match_hooks(HookEvent::PreToolUse, None);
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].event, HookEvent::PreToolUse);
}

#[test]
fn test_match_with_tool_filter() {
    let reg = HookRegistry::new(vec![make_hook(
        HookEvent::PreToolUse,
        Some(vec!["bash".into(), "write".into()]),
    )]);
    assert_eq!(reg.match_hooks(HookEvent::PreToolUse, Some("bash")).len(), 1);
    assert_eq!(reg.match_hooks(HookEvent::PreToolUse, Some("read")).len(), 0);
    assert_eq!(reg.match_hooks(HookEvent::PreToolUse, None).len(), 0);
}

#[test]
fn test_no_match_wrong_event() {
    let reg = HookRegistry::new(vec![make_hook(HookEvent::PreToolUse, None)]);
    assert!(reg.match_hooks(HookEvent::PostToolUse, None).is_empty());
}

#[test]
fn test_no_filter_matches_any_tool() {
    let reg = HookRegistry::new(vec![make_hook(HookEvent::PreToolUse, None)]);
    assert_eq!(
        reg.match_hooks(HookEvent::PreToolUse, Some("anything")).len(),
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
    let matched = reg.match_hooks(HookEvent::PreToolUse, None);
    assert_eq!(matched.len(), 2);
}

#[test]
fn test_match_hooks_with_tool_filter_only_matches_specified_tools() {
    let reg = HookRegistry::new(vec![
        make_hook(
            HookEvent::PreToolUse,
            Some(vec!["bash".into()]),
        ),
        make_hook(
            HookEvent::PreToolUse,
            Some(vec!["write".into(), "edit".into()]),
        ),
    ]);
    // "bash" matches only the first hook
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("bash"));
    assert_eq!(matched.len(), 1);

    // "write" matches only the second hook
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("write"));
    assert_eq!(matched.len(), 1);

    // "edit" matches only the second hook
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("edit"));
    assert_eq!(matched.len(), 1);

    // "unknown" matches neither
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("unknown"));
    assert_eq!(matched.len(), 0);
}

#[test]
fn test_empty_registry_returns_empty() {
    let reg = HookRegistry::new(vec![]);
    assert!(reg.match_hooks(HookEvent::PreToolUse, None).is_empty());
    assert!(reg
        .match_hooks(HookEvent::PreToolUse, Some("bash"))
        .is_empty());
}

#[test]
fn test_mixed_filtered_and_unfiltered_hooks() {
    let reg = HookRegistry::new(vec![
        make_hook(HookEvent::PreToolUse, None),           // matches any tool
        make_hook(HookEvent::PreToolUse, Some(vec!["bash".into()])), // only bash
    ]);

    // With tool_name "bash", both should match
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("bash"));
    assert_eq!(matched.len(), 2);

    // With tool_name "read", only the unfiltered one matches
    let matched = reg.match_hooks(HookEvent::PreToolUse, Some("read"));
    assert_eq!(matched.len(), 1);

    // With None tool_name, only the unfiltered one matches (filtered requires a tool name)
    let matched = reg.match_hooks(HookEvent::PreToolUse, None);
    assert_eq!(matched.len(), 1);
}
