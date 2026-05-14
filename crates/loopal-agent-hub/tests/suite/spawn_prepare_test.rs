use loopal_agent_hub::dispatch::spawn_prepare::prepare_remote_spawn_args;
use serde_json::json;
use std::path::Path;

fn cwd(p: &str) -> &Path {
    Path::new(p)
}

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
    let args = prepare_remote_spawn_args(
        &json!({
            "name": "child",
            "prompt": "do work",
            "model": "claude-opus-4-7",
            "permission_mode": "ask_dangerous",
            "decision_mode": "classifier",
            "agent_type": "explore",
            "depth": 3,
        }),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.model.as_deref(), Some("claude-opus-4-7"));
    assert_eq!(
        args.permission_mode.as_deref(),
        Some("bypass"),
        "non-clamped permission_mode must be overridden to bypass"
    );
    assert_eq!(
        args.decision_mode.as_deref(),
        Some("manual"),
        "non-clamped decision_mode must be overridden to manual"
    );
    assert_eq!(args.agent_type.as_deref(), Some("explore"));
    assert_eq!(args.depth, Some(3));
    assert_eq!(args.prompt.as_deref(), Some("do work"));
}

#[test]
fn permission_defaults_to_bypass_manual_when_omitted() {
    let args = prepare_remote_spawn_args(&json!({"name": "child"}), "caller", cwd("/cwd")).unwrap();
    assert_eq!(
        args.permission_mode.as_deref(),
        Some("bypass"),
        "missing permission_mode must default to bypass"
    );
    assert_eq!(
        args.decision_mode.as_deref(),
        Some("manual"),
        "missing decision_mode must default to manual"
    );
}

#[test]
fn permission_keeps_bypass_manual_when_caller_already_clamped() {
    let args = prepare_remote_spawn_args(
        &json!({
            "name": "child",
            "permission_mode": "bypass",
            "decision_mode": "manual",
        }),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.permission_mode.as_deref(), Some("bypass"));
    assert_eq!(args.decision_mode.as_deref(), Some("manual"));
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
