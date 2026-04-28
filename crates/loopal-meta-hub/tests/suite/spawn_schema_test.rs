//! Tests for `meta/spawn` schema validation — verifies that the MetaHub
//! rejects payloads carrying filesystem-coupled fields before forwarding.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_meta_hub::MetaHub;
use loopal_meta_hub::dispatch::dispatch_meta_request;
use serde_json::json;

#[tokio::test]
async fn meta_spawn_rejects_cwd() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let result = dispatch_meta_request(
        &meta_hub,
        "meta/spawn",
        json!({
            "name": "child",
            "prompt": "test",
            "target_hub": "hub-b",
            "cwd": "/attacker/path",
        }),
        "hub-a".into(),
    )
    .await;
    let err = result.expect_err("meta/spawn must reject cwd");
    assert!(err.contains("cwd"), "got: {err}");
}

#[tokio::test]
async fn meta_spawn_rejects_fork_context() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let result = dispatch_meta_request(
        &meta_hub,
        "meta/spawn",
        json!({
            "name": "child",
            "prompt": "test",
            "target_hub": "hub-b",
            "fork_context": [],
        }),
        "hub-a".into(),
    )
    .await;
    let err = result.expect_err("meta/spawn must reject fork_context");
    assert!(err.contains("fork_context"), "got: {err}");
}

#[tokio::test]
async fn meta_spawn_rejects_resume() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let result = dispatch_meta_request(
        &meta_hub,
        "meta/spawn",
        json!({
            "name": "child",
            "prompt": "test",
            "target_hub": "hub-b",
            "resume": "session-123",
        }),
        "hub-a".into(),
    )
    .await;
    let err = result.expect_err("meta/spawn must reject resume");
    assert!(err.contains("resume"), "got: {err}");
}

#[tokio::test]
async fn meta_spawn_rejects_when_target_hub_missing() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let result = dispatch_meta_request(
        &meta_hub,
        "meta/spawn",
        json!({"name": "child", "prompt": "test"}),
        "hub-a".into(),
    )
    .await;
    let err = result.expect_err("must reject without target_hub");
    assert!(err.contains("target_hub"), "got: {err}");
}

#[tokio::test]
async fn meta_spawn_rejects_unknown_target_hub() {
    // Without forbidden fields and with a target_hub that is not registered,
    // the call should fail at the connection lookup — proving the validation
    // checks completed and we got past them.
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let result = dispatch_meta_request(
        &meta_hub,
        "meta/spawn",
        json!({
            "name": "child",
            "prompt": "test",
            "target_hub": "no-such-hub",
        }),
        "hub-a".into(),
    )
    .await;
    let err = result.expect_err("unregistered hub must fail");
    assert!(
        err.contains("not connected"),
        "expected connection lookup failure, got: {err}"
    );
}
