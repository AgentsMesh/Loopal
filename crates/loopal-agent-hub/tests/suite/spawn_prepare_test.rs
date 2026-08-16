use loopal_agent_hub::dispatch::spawn_prepare::prepare_remote_spawn_args;
use serde_json::{Value, json};

fn valid() -> Value {
    json!({
        "name": "child",
        "model": "worker-model",
        "prompt": "do work",
        "parent": "origin/parent",
        "depth": 3,
        "permission_mode": "ask_dangerous",
        "decision_mode": "classifier",
        "sandbox_policy": "read_only",
        "no_sandbox": false,
        "agent_type": "explore",
    })
}

#[test]
fn preserves_stamped_authority_and_destination_cwd() {
    let root = tempfile::tempdir().unwrap();
    let args = prepare_remote_spawn_args(&valid(), root.path()).unwrap();
    assert_eq!(args.cwd, root.path().canonicalize().unwrap());
    assert_eq!(args.name, "child");
    assert_eq!(args.model, "worker-model");
    assert_eq!(args.prompt.as_deref(), Some("do work"));
    assert_eq!(args.parent, "origin/parent");
    assert_eq!(args.depth, 3);
    assert_eq!(args.permission_mode, "ask_dangerous");
    assert_eq!(args.decision_mode, "classifier");
    assert_eq!(args.sandbox_policy, "read_only");
    assert!(!args.no_sandbox);
    assert_eq!(args.agent_type.as_deref(), Some("explore"));
}

#[test]
fn accepts_consistent_disabled_sandbox() {
    let root = tempfile::tempdir().unwrap();
    let mut params = valid();
    params["sandbox_policy"] = json!("disabled");
    params["no_sandbox"] = json!(true);
    let args = prepare_remote_spawn_args(&params, root.path()).unwrap();
    assert_eq!(args.sandbox_policy, "disabled");
    assert!(args.no_sandbox);
}

#[test]
fn rejects_each_missing_required_field() {
    let root = tempfile::tempdir().unwrap();
    for field in [
        "name",
        "model",
        "parent",
        "depth",
        "permission_mode",
        "decision_mode",
        "sandbox_policy",
        "no_sandbox",
    ] {
        let mut params = valid();
        params.as_object_mut().unwrap().remove(field);
        let error = prepare_remote_spawn_args(&params, root.path()).unwrap_err();
        assert!(error.contains(field), "{field}: {error}");
    }
}

#[test]
fn rejects_invalid_parent_and_depth() {
    let root = tempfile::tempdir().unwrap();
    for (field, value) in [
        ("parent", json!("local-parent")),
        ("parent", json!("//attacker")),
        ("depth", json!(0)),
        ("depth", json!(-1)),
        ("depth", json!(u64::from(u32::MAX) + 1)),
    ] {
        let mut params = valid();
        params[field] = value;
        assert!(prepare_remote_spawn_args(&params, root.path()).is_err());
    }
}

#[test]
fn rejects_unknown_or_conflicting_policy_values() {
    let root = tempfile::tempdir().unwrap();
    for (field, value) in [
        ("permission_mode", json!("unknown")),
        ("decision_mode", json!("unknown")),
        ("sandbox_policy", json!("unknown")),
        ("no_sandbox", json!("false")),
    ] {
        let mut params = valid();
        params[field] = value;
        assert!(prepare_remote_spawn_args(&params, root.path()).is_err());
    }
    let mut conflict = valid();
    conflict["no_sandbox"] = json!(true);
    assert!(prepare_remote_spawn_args(&conflict, root.path()).is_err());
}

#[test]
fn rejects_filesystem_coupled_fields() {
    let root = tempfile::tempdir().unwrap();
    for field in ["cwd", "fork_context", "resume"] {
        let mut params = valid();
        params[field] = json!("attacker");
        let error = prepare_remote_spawn_args(&params, root.path()).unwrap_err();
        assert!(error.contains(field), "{field}: {error}");
    }
}

#[test]
fn rejects_non_object_and_bad_optional_types() {
    let root = tempfile::tempdir().unwrap();
    assert!(prepare_remote_spawn_args(&json!(null), root.path()).is_err());
    for field in ["prompt", "agent_type"] {
        let mut params = valid();
        params[field] = json!(42);
        assert!(prepare_remote_spawn_args(&params, root.path()).is_err());
    }
}

#[test]
fn rejects_missing_destination_cwd() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    assert!(prepare_remote_spawn_args(&valid(), &missing).is_err());
}
