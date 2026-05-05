//! Unit tests for `build_spawn_request` — the InHub / CrossHub field selection.

use std::path::PathBuf;

use loopal_agent::spawn::{SpawnParams, SpawnTarget, build_spawn_request};

fn base_params(target: SpawnTarget) -> SpawnParams {
    SpawnParams {
        name: "child".into(),
        prompt: "do work".into(),
        model: Some("claude-opus-4-7".into()),
        permission_mode: Some("supervised".into()),
        agent_type: None,
        depth: 1,
        no_sandbox: false,
        target,
    }
}

#[test]
fn inhub_uses_parent_cwd_when_no_override() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let params = base_params(SpawnTarget::InHub {
        cwd_override: None,
        fork_context: None,
    });
    let req = build_spawn_request(&params, &parent_cwd);
    assert_eq!(req["cwd"], "/parent/dir");
    assert!(req.get("target_hub").is_none());
    assert!(req.get("fork_context").is_none());
}

#[test]
fn inhub_uses_cwd_override_when_provided() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let params = base_params(SpawnTarget::InHub {
        cwd_override: Some(PathBuf::from("/wt/branch-x")),
        fork_context: None,
    });
    let req = build_spawn_request(&params, &parent_cwd);
    assert_eq!(req["cwd"], "/wt/branch-x");
}

#[test]
fn inhub_serializes_fork_context() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let messages = vec![loopal_message::Message::user("earlier")];
    let params = base_params(SpawnTarget::InHub {
        cwd_override: None,
        fork_context: Some(messages),
    });
    let req = build_spawn_request(&params, &parent_cwd);
    assert!(req["fork_context"].is_array());
    assert_eq!(req["fork_context"].as_array().unwrap().len(), 1);
}

#[test]
fn crosshub_omits_cwd_and_fork_context() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let params = base_params(SpawnTarget::CrossHub {
        hub_id: "hub-b".into(),
    });
    let req = build_spawn_request(&params, &parent_cwd);
    assert!(
        req.get("cwd").is_none(),
        "cross-hub spawn must not carry parent cwd"
    );
    assert!(
        req.get("fork_context").is_none(),
        "cross-hub spawn must not carry fork_context"
    );
    assert_eq!(req["target_hub"], "hub-b");
}

#[test]
fn crosshub_still_carries_advisory_fields() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let params = base_params(SpawnTarget::CrossHub {
        hub_id: "hub-b".into(),
    });
    let req = build_spawn_request(&params, &parent_cwd);
    // permission_mode / model / agent_type / depth are advisory hints — receiver
    // policy is the enforcement point — but they must still be transmitted.
    assert_eq!(req["permission_mode"], "supervised");
    assert_eq!(req["model"], "claude-opus-4-7");
    assert_eq!(req["depth"], 1);
}

#[test]
fn inhub_propagates_no_sandbox_true() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let mut params = base_params(SpawnTarget::InHub {
        cwd_override: None,
        fork_context: None,
    });
    params.no_sandbox = true;
    let req = build_spawn_request(&params, &parent_cwd);
    assert_eq!(req["no_sandbox"], true);
}

#[test]
fn inhub_propagates_no_sandbox_false() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let params = base_params(SpawnTarget::InHub {
        cwd_override: None,
        fork_context: None,
    });
    let req = build_spawn_request(&params, &parent_cwd);
    assert_eq!(req["no_sandbox"], false);
}

#[test]
fn crosshub_propagates_no_sandbox() {
    let parent_cwd = PathBuf::from("/parent/dir");
    let mut params = base_params(SpawnTarget::CrossHub {
        hub_id: "hub-b".into(),
    });
    params.no_sandbox = true;
    let req = build_spawn_request(&params, &parent_cwd);
    // Behavior flag — not filesystem-coupled, so it crosses hub boundaries.
    assert_eq!(req["no_sandbox"], true);
}
