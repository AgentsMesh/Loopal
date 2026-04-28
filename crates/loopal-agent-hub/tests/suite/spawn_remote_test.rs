//! Tests for `hub/spawn_remote_agent` handler — verifies cross-hub spawn
//! safety invariants: forbidden filesystem-coupled fields are rejected,
//! receiver Hub's `default_cwd` is used (not the caller's).

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::dispatch::dispatch_hub_request;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub_with_cwd(cwd: PathBuf) -> Arc<Mutex<Hub>> {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(16);
    Arc::new(Mutex::new(Hub::with_cwd(tx, cwd)))
}

#[tokio::test]
async fn rejects_cwd_field() {
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_remote_agent",
        json!({
            "name": "child",
            "prompt": "report cwd",
            "cwd": "/attacker/path",
        }),
        "from-agent".into(),
    )
    .await;
    let err = result.expect_err("must reject cwd in cross-hub spawn");
    assert!(err.contains("cwd"), "error must mention forbidden 'cwd' field, got: {err}");
}

#[tokio::test]
async fn rejects_fork_context_field() {
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_remote_agent",
        json!({
            "name": "child",
            "prompt": "do work",
            "fork_context": [],
        }),
        "from-agent".into(),
    )
    .await;
    let err = result.expect_err("must reject fork_context");
    assert!(
        err.contains("fork_context"),
        "error must mention 'fork_context', got: {err}"
    );
}

#[tokio::test]
async fn rejects_resume_field() {
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_remote_agent",
        json!({
            "name": "child",
            "prompt": "do work",
            "resume": "session-123",
        }),
        "from-agent".into(),
    )
    .await;
    let err = result.expect_err("must reject session resume");
    assert!(err.contains("resume"), "error must mention 'resume', got: {err}");
}

#[tokio::test]
async fn rejects_when_name_missing() {
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_remote_agent",
        json!({"prompt": "do work"}),
        "from-agent".into(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn local_spawn_path_unaffected_by_remote_handler() {
    // Verify the new method is wired distinctly: a regular hub/spawn_agent
    // request without target_hub still reaches the local handler (not the
    // remote one). Failure here would surface as a different error type.
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    // No name provided — local handler returns "missing 'name' field".
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_agent",
        json!({"prompt": "x"}),
        "from-agent".into(),
    )
    .await;
    let err = result.expect_err("missing name");
    assert!(err.contains("name"), "got: {err}");
}

#[tokio::test]
async fn rejects_non_string_target_hub() {
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_agent",
        json!({"name": "child", "prompt": "x", "target_hub": 42}),
        "from-agent".into(),
    )
    .await;
    let err = result.expect_err("non-string target_hub must be rejected");
    assert!(
        err.contains("target_hub") && err.contains("string"),
        "error must point to type mismatch, got: {err}"
    );
}

#[tokio::test]
async fn cross_hub_forward_rejects_slash_in_child_name() {
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_agent",
        json!({
            "name": "foo/bar",
            "prompt": "x",
            "target_hub": "hub-b",
        }),
        "main".into(),
    )
    .await;
    let err = result.expect_err("'/' in child name must be rejected");
    assert!(
        err.contains("'/'") || err.contains("cannot contain"),
        "error must mention slash restriction, got: {err}"
    );
}

#[tokio::test]
async fn cross_hub_forward_rejects_slash_in_caller_name() {
    let hub = make_hub_with_cwd(PathBuf::from("/hub-local"));
    let result = dispatch_hub_request(
        &hub,
        "hub/spawn_agent",
        json!({
            "name": "child",
            "prompt": "x",
            "target_hub": "hub-b",
        }),
        "main/branch".into(),
    )
    .await;
    let err = result.expect_err("'/' in caller name must be rejected");
    assert!(
        err.contains("caller") && (err.contains("'/'") || err.contains("cannot contain")),
        "error must point to caller name slash, got: {err}"
    );
}
