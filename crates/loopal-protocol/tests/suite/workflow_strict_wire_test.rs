use loopal_protocol::*;

use crate::workflow_support::text_spec;

fn assert_unknown_rejected(path: &[&str], field: &str) {
    let mut value = serde_json::to_value(text_spec()).unwrap();
    let mut target = &mut value;
    for segment in path {
        target = if let Ok(index) = segment.parse::<usize>() {
            &mut target.as_array_mut().unwrap()[index]
        } else {
            target.get_mut(*segment).unwrap()
        };
    }
    target
        .as_object_mut()
        .unwrap()
        .insert(field.into(), serde_json::json!("forbidden"));
    assert!(
        serde_json::from_value::<WorkflowSpec>(value).is_err(),
        "unknown {field} at {path:?} was accepted"
    );
}

#[test]
fn v1_spec_rejects_unknown_authority_and_extension_fields() {
    for field in [
        "cwd",
        "depth",
        "permission_mode",
        "decision_mode",
        "sandbox",
        "secrets",
    ] {
        assert_unknown_rejected(&[], field);
        assert_unknown_rejected(&["nodes", "0"], field);
    }
}

#[test]
fn v1_limits_and_output_contract_reject_unknown_fields() {
    for field in ["cpu", "token_budget", "connection_generation"] {
        assert_unknown_rejected(&["limits"], field);
    }
    for field in ["artifact", "validator", "schema_version"] {
        assert_unknown_rejected(&["output_contract"], field);
    }
}

#[test]
fn strict_wire_still_roundtrips_the_declared_shape() {
    let spec = text_spec();
    let value = serde_json::to_value(&spec).unwrap();
    assert_eq!(serde_json::from_value::<WorkflowSpec>(value).unwrap(), spec);
}
