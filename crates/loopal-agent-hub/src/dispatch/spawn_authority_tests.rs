use std::path::PathBuf;

use loopal_config::SandboxPolicy;
use loopal_decision_api::DecisionMode;
use loopal_protocol::QualifiedAddress;
use loopal_tool_api::PermissionMode;
use serde_json::json;

use super::spawn_authority::{prepare_cross_hub_payload, prepare_local};
use crate::request_principal::AgentPrincipal;
use crate::types::{AgentExecutionRef, AgentOrigin, SpawnAuthority};

fn principal(root: PathBuf) -> AgentPrincipal {
    AgentPrincipal {
        execution: AgentExecutionRef::local("parent", 7),
        origin: AgentOrigin::ManagedChild,
        cwd: root.clone(),
        root_cwd: root,
        root: "main".into(),
        depth: 1,
        session_id: None,
        workflow_permission_causation: None,
        spawn: SpawnAuthority {
            model: "model-a".into(),
            permission_mode: PermissionMode::AskAnyWrite,
            decision_mode: DecisionMode::Classifier,
            sandbox_policy: SandboxPolicy::ReadOnly,
        },
    }
}

#[test]
fn local_spawn_derives_exact_authority() {
    let root = tempfile::tempdir().unwrap();
    let caller = principal(root.path().canonicalize().unwrap());
    let prepared = prepare_local(
        &json!({
            "name": "worker",
            "cwd": root.path(),
            "depth": 2,
            "permission_mode": "ask_any_write",
            "decision_mode": "classifier",
            "no_sandbox": false,
            "model": "worker-model",
        }),
        &caller,
        3,
    )
    .unwrap();
    assert_eq!(prepared.parent, Some(QualifiedAddress::local("parent")));
    assert_eq!(prepared.parent_execution, Some(caller.execution));
    assert_eq!(prepared.depth, 2);
    assert_eq!(prepared.authority.model, "worker-model");
    assert_eq!(prepared.authority.sandbox_policy, SandboxPolicy::ReadOnly);
}

#[test]
fn local_spawn_rejects_authority_escalation() {
    let root = tempfile::tempdir().unwrap();
    let caller = principal(root.path().canonicalize().unwrap());
    for params in [
        json!({"name": "worker", "parent": "main"}),
        json!({"name": "worker", "depth": 0}),
        json!({"name": "worker", "permission_mode": "bypass"}),
        json!({"name": "worker", "decision_mode": "manual"}),
        json!({"name": "worker", "no_sandbox": true}),
        json!({"name": "worker", "sandbox_policy": "disabled"}),
        json!({"name": "worker", "session_id": "forged"}),
        json!({"name": "worker", "resume": "forged"}),
        json!({"name": "worker", "lifecycle": "persistent"}),
    ] {
        assert!(prepare_local(&params, &caller, 3).is_err(), "{params}");
    }
}

#[test]
fn local_spawn_resolves_relative_cwd_from_authenticated_parent() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("child")).unwrap();
    let caller = principal(root.path().canonicalize().unwrap());
    let prepared = prepare_local(&json!({"name": "worker", "cwd": "child"}), &caller, 3).unwrap();
    assert_eq!(
        prepared.cwd,
        root.path().join("child").canonicalize().unwrap()
    );
}

#[test]
fn local_spawn_rejects_outside_and_symlink_escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let caller = principal(root.path().canonicalize().unwrap());
    assert!(
        prepare_local(
            &json!({"name": "worker", "cwd": outside.path()}),
            &caller,
            3,
        )
        .is_err()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        assert!(
            prepare_local(
                &json!({"name": "worker", "cwd": root.path().join("escape")}),
                &caller,
                3,
            )
            .is_err()
        );
    }
}

#[test]
fn local_spawn_rejects_depth_limit_before_effect() {
    let root = tempfile::tempdir().unwrap();
    let caller = principal(root.path().canonicalize().unwrap());
    assert!(prepare_local(&json!({"name": "worker"}), &caller, 1).is_err());
}

#[test]
fn cross_hub_payload_is_stamped_from_principal() {
    let root = tempfile::tempdir().unwrap();
    let caller = principal(root.path().canonicalize().unwrap());
    let value = prepare_cross_hub_payload(
        &json!({"name": "worker", "prompt": "work", "depth": 2}),
        &caller,
        3,
    )
    .unwrap();
    assert_eq!(value["permission_mode"], "ask_any_write");
    assert_eq!(value["decision_mode"], "classifier");
    assert_eq!(value["sandbox_policy"], "read_only");
    assert_eq!(value["no_sandbox"], false);
    assert_eq!(value["depth"], 2);
}

#[test]
fn cross_hub_payload_rejects_forged_or_filesystem_authority() {
    let root = tempfile::tempdir().unwrap();
    let caller = principal(root.path().canonicalize().unwrap());
    for params in [
        json!({"name": "worker", "parent": "other/agent"}),
        json!({"name": "worker", "cwd": "/tmp"}),
        json!({"name": "worker", "permission_mode": "bypass"}),
        json!({"name": "worker", "no_sandbox": true}),
        json!({"name": "worker", "sandbox_policy": "disabled"}),
        json!({"name": "worker", "session_id": "forged"}),
        json!({"name": "worker", "lifecycle": "persistent"}),
    ] {
        assert!(prepare_cross_hub_payload(&params, &caller, 3).is_err());
    }
}
