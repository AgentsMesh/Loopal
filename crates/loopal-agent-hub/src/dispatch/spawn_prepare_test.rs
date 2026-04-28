//! Unit tests for `prepare_remote_spawn_args`. Kept in a sibling file to
//! respect the 200-line module limit.

use super::spawn_prepare::prepare_remote_spawn_args;
use serde_json::json;
use std::path::Path;

fn cwd(p: &str) -> &Path {
    Path::new(p)
}

#[test]
fn uses_receiver_default_cwd_not_caller_input() {
    // Anchor for the cross-hub invariant: the receiver's default_cwd must
    // reach spawn_manager regardless of what (if anything) the caller sent —
    // preventing caller cwd leaks into receiver system prompt.
    let args = prepare_remote_spawn_args(
        &json!({"name": "child", "prompt": "x"}),
        "caller",
        cwd("/receiver/forced"),
    )
    .unwrap();
    assert_eq!(args.cwd, "/receiver/forced");
}

#[test]
fn advisory_fields_are_propagated() {
    let args = prepare_remote_spawn_args(
        &json!({
            "name": "child",
            "prompt": "do work",
            "model": "claude-opus-4-7",
            "permission_mode": "supervised",
            "agent_type": "explore",
            "depth": 3,
        }),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.model.as_deref(), Some("claude-opus-4-7"));
    assert_eq!(args.permission_mode.as_deref(), Some("supervised"));
    assert_eq!(args.agent_type.as_deref(), Some("explore"));
    assert_eq!(args.depth, Some(3));
    assert_eq!(args.prompt.as_deref(), Some("do work"));
}

#[test]
fn parent_falls_back_to_from_agent_when_unset() {
    let args = prepare_remote_spawn_args(
        &json!({"name": "child"}),
        "the-caller",
        cwd("/cwd"),
    )
    .unwrap();
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
    // A bare local address like "main" must not pass — caller side should
    // always send a fully-qualified `hub/agent` for cross-hub spawn.
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
    // Empty segments make QualifiedAddress::parse silently fall back to a
    // local address — must be rejected to prevent delivery to an
    // unintended local agent.
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
    // Malicious caller sending depth: 0 must not produce a "root-like"
    // child that bypasses the receiver's depth-based tool filter.
    let args = prepare_remote_spawn_args(
        &json!({"name": "child", "depth": 0}),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.depth, Some(1));
}

#[test]
fn depth_above_one_passes_through() {
    let args = prepare_remote_spawn_args(
        &json!({"name": "child", "depth": 5}),
        "caller",
        cwd("/cwd"),
    )
    .unwrap();
    assert_eq!(args.depth, Some(5));
}

#[test]
fn rejects_cwd() {
    let err = prepare_remote_spawn_args(
        &json!({"name": "x", "cwd": "/attacker"}),
        "f",
        cwd("/c"),
    )
    .unwrap_err();
    assert!(err.contains("cwd"));
}

#[test]
fn rejects_fork_context() {
    let err = prepare_remote_spawn_args(
        &json!({"name": "x", "fork_context": []}),
        "f",
        cwd("/c"),
    )
    .unwrap_err();
    assert!(err.contains("fork_context"));
}

#[test]
fn rejects_resume() {
    let err = prepare_remote_spawn_args(
        &json!({"name": "x", "resume": "session-1"}),
        "f",
        cwd("/c"),
    )
    .unwrap_err();
    assert!(err.contains("resume"));
}

#[test]
fn rejects_when_name_missing() {
    let err =
        prepare_remote_spawn_args(&json!({"prompt": "x"}), "f", cwd("/c")).unwrap_err();
    assert!(err.contains("name"));
}
