use serde_json::{Value, json};

use super::validate_forwarded_spawn_payload;

fn valid() -> Value {
    json!({
        "name": "child",
        "model": "model",
        "parent": "origin/main",
        "depth": 1,
        "permission_mode": "ask_any_write",
        "decision_mode": "classifier",
        "sandbox_policy": "read_only",
        "no_sandbox": false,
    })
}

#[test]
fn complete_payload_passes() {
    assert!(validate_forwarded_spawn_payload(&valid()).is_ok());
}

#[test]
fn every_required_field_is_fail_closed() {
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
        let error = validate_forwarded_spawn_payload(&params).unwrap_err();
        assert!(error.contains(field), "{field}: {error}");
    }
}

#[test]
fn rejects_empty_identity_strings() {
    for field in ["name", "model", "parent"] {
        let mut params = valid();
        params[field] = json!("  ");
        assert!(validate_forwarded_spawn_payload(&params).is_err());
    }
}

#[test]
fn depth_must_be_positive_u32() {
    for value in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(u64::from(u32::MAX) + 1),
    ] {
        let mut params = valid();
        params["depth"] = value;
        assert!(validate_forwarded_spawn_payload(&params).is_err());
    }
}

#[test]
fn policy_enums_are_closed() {
    for field in ["permission_mode", "decision_mode", "sandbox_policy"] {
        let mut params = valid();
        params[field] = json!("future_value");
        let error = validate_forwarded_spawn_payload(&params).unwrap_err();
        assert!(error.contains(field), "{field}: {error}");
    }
}

#[test]
fn sandbox_fields_must_agree() {
    let mut enabled_conflict = valid();
    enabled_conflict["no_sandbox"] = json!(true);
    assert!(validate_forwarded_spawn_payload(&enabled_conflict).is_err());

    let mut disabled_conflict = valid();
    disabled_conflict["sandbox_policy"] = json!("disabled");
    assert!(validate_forwarded_spawn_payload(&disabled_conflict).is_err());

    disabled_conflict["no_sandbox"] = json!(true);
    assert!(validate_forwarded_spawn_payload(&disabled_conflict).is_ok());
}

#[test]
fn malformed_shape_and_forbidden_fields_fail() {
    assert!(validate_forwarded_spawn_payload(&json!(null)).is_err());
    for field in ["cwd", "fork_context", "resume"] {
        let mut params = valid();
        params[field] = json!("forged");
        assert!(validate_forwarded_spawn_payload(&params).is_err());
    }
}
