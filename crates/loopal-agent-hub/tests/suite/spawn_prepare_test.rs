use loopal_agent_hub::dispatch::spawn_prepare::prepare_remote_spawn_args;
use serde_json::json;
use std::path::Path;

fn cwd(p: &str) -> &Path {
    Path::new(p)
}

const BYPASS_MANUAL: &str = r#"{"decision":"manual","mode":"bypass"}"#;

#[test]
fn uses_receiver_default_cwd_not_caller_input() {
    let args = prepare_remote_spawn_args(
        &json!({"name": "child", "prompt": "x"}),
        "caller",
        cwd("/receiver/forced"),
    )
    .unwrap();
    assert_eq!(args.cwd, "/receiver/forced");
}

#[test]
fn permission_clamps_to_bypass_manual_when_caller_sends_non_clamped() {
    // Cross-hub agents are headless; any non-Bypass/Manual would manifest as
    // 30s timeout denials. Receiver overrides whatever the caller sent.
    let args = prepare_remote_spawn_args(
        &json!({
            "name": "child",
            "prompt": "do work",
            "model": "claude-opus-4-7",
            "permission": r#"{"mode":"ask_dangerous","decision":"auto"}"#,
            "agent_type": "explore",
            "depth": 3,
        }),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.model.as_deref(), Some("claude-opus-4-7"));
    assert_eq!(
        args.permission.as_deref(),
        Some(BYPASS_MANUAL),
        "non-clamped permission must be overridden to bypass+manual"
    );
    assert_eq!(args.agent_type.as_deref(), Some("explore"));
    assert_eq!(args.depth, Some(3));
    assert_eq!(args.prompt.as_deref(), Some("do work"));
}

#[test]
fn permission_defaults_to_bypass_manual_when_omitted() {
    let args = prepare_remote_spawn_args(&json!({"name": "child"}), "caller", cwd("/cwd")).unwrap();
    assert_eq!(
        args.permission.as_deref(),
        Some(BYPASS_MANUAL),
        "missing permission must default to bypass+manual"
    );
}

#[test]
fn permission_keeps_bypass_manual_when_caller_already_clamped() {
    let args = prepare_remote_spawn_args(
        &json!({
            "name": "child",
            "permission": BYPASS_MANUAL,
        }),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.permission.as_deref(), Some(BYPASS_MANUAL));
}

#[test]
fn parent_falls_back_to_from_agent_when_unset() {
    let args =
        prepare_remote_spawn_args(&json!({"name": "child"}), "the-caller", cwd("/cwd")).unwrap();
    assert_eq!(args.parent.as_deref(), Some("the-caller"));
}

#[test]
fn parent_uses_explicit_value_when_provided() {
    let args = prepare_remote_spawn_args(
        &json!({"name": "child", "parent": "hub-a/grandparent"}),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.parent.as_deref(), Some("hub-a/grandparent"));
}

#[test]
fn parent_rejects_local_form() {
    let err = prepare_remote_spawn_args(
        &json!({"name": "child", "parent": "main"}),
        "caller",
        cwd("/cwd"),
    )
    .unwrap_err();
    assert!(err.contains("parent"), "got: {err}");
}

#[test]
fn parent_rejects_empty_segment() {
    let err = prepare_remote_spawn_args(
        &json!({"name": "child", "parent": "//attacker"}),
        "caller",
        cwd("/cwd"),
    )
    .unwrap_err();
    assert!(err.contains("parent"), "got: {err}");
}

#[test]
fn depth_zero_clamps_to_one() {
    let args =
        prepare_remote_spawn_args(&json!({"name": "child", "depth": 0}), "caller", cwd("/cwd"))
            .unwrap();
    assert_eq!(args.depth, Some(1));
}

#[test]
fn depth_above_one_passes_through() {
    let args =
        prepare_remote_spawn_args(&json!({"name": "child", "depth": 5}), "caller", cwd("/cwd"))
            .unwrap();
    assert_eq!(args.depth, Some(5));
}

#[test]
fn rejects_cwd() {
    let err = prepare_remote_spawn_args(&json!({"name": "x", "cwd": "/attacker"}), "f", cwd("/c"))
        .unwrap_err();
    assert!(err.contains("cwd"));
}

#[test]
fn rejects_fork_context() {
    let err = prepare_remote_spawn_args(&json!({"name": "x", "fork_context": []}), "f", cwd("/c"))
        .unwrap_err();
    assert!(err.contains("fork_context"));
}

#[test]
fn rejects_resume() {
    let err =
        prepare_remote_spawn_args(&json!({"name": "x", "resume": "session-1"}), "f", cwd("/c"))
            .unwrap_err();
    assert!(err.contains("resume"));
}

#[test]
fn rejects_when_name_missing() {
    let err = prepare_remote_spawn_args(&json!({"prompt": "x"}), "f", cwd("/c")).unwrap_err();
    assert!(err.contains("name"));
}

#[test]
fn no_sandbox_passed_through_when_present() {
    let args = prepare_remote_spawn_args(
        &json!({"name": "child", "no_sandbox": true}),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert!(args.no_sandbox);
}

#[test]
fn no_sandbox_defaults_false_when_missing() {
    let args = prepare_remote_spawn_args(&json!({"name": "child"}), "caller", cwd("/cwd")).unwrap();
    assert!(!args.no_sandbox);
}
