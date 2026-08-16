use std::str::FromStr;

use loopal_protocol::{
    PermissionActionDigest, PermissionDisplayDigest, PermissionIntent, PermissionIntentSeed,
    PermissionSchemaDigest, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};
use serde_json::{Value, json};

fn action(byte: u8) -> PermissionActionDigest {
    PermissionActionDigest::from_bytes([byte; 32])
}

fn display(byte: u8) -> PermissionDisplayDigest {
    PermissionDisplayDigest::from_bytes([byte; 32])
}

fn schema(byte: u8) -> PermissionSchemaDigest {
    PermissionSchemaDigest::from_bytes([byte; 32])
}

fn workflow() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::from("wrun_1"),
        node_id: WorkflowNodeId::from("wnode_1"),
        attempt_id: WorkflowAttemptId::from("watt_1"),
    }
}

fn intent() -> PermissionIntent {
    PermissionIntent::bind(
        PermissionIntentSeed::new(
            "Bash",
            action(0x11),
            display(0x33),
            schema(0x22),
            Some(workflow()),
        )
        .unwrap(),
        7,
        9,
        "interaction-1",
    )
    .unwrap()
}

#[test]
fn typed_digest_wire_is_strict_lowercase_sha256() {
    let encoded = action(0xab).to_string();
    assert_eq!(encoded, format!("sha256:{}", "ab".repeat(32)));
    assert_eq!(
        encoded.parse::<PermissionActionDigest>().unwrap(),
        action(0xab)
    );
    for invalid in [
        "ab".repeat(32),
        format!("sha256:{}", "AB".repeat(32)),
        format!("sha256:{}", "a".repeat(63)),
        format!("sha256:{}g", "a".repeat(63)),
    ] {
        assert!(
            PermissionActionDigest::from_str(&invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn intent_roundtrip_is_stable_and_binds_every_field() {
    let original = intent();
    let value = serde_json::to_value(&original).unwrap();
    let decoded: PermissionIntent = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(decoded.seed().tool_name(), "Bash");
    assert_eq!(decoded.execution_generation(), 7);
    assert_eq!(decoded.ui_generation(), 9);
    assert_eq!(decoded.interaction_token(), "interaction-1");

    let same = intent();
    assert_eq!(same.intent_digest(), original.intent_digest());
    let changed = PermissionIntent::bind(original.seed().clone(), 8, 9, "interaction-1").unwrap();
    assert_ne!(changed.intent_digest(), original.intent_digest());
    assert!(
        value["intent_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

#[test]
fn tampering_any_bound_field_or_digest_is_rejected() {
    let value = serde_json::to_value(intent()).unwrap();
    for (field, replacement) in [
        ("tool_name", json!("Read")),
        ("execution_generation", json!(8)),
        ("ui_generation", json!(10)),
        ("interaction_token", json!("interaction-2")),
        ("action_digest", json!(action(0x33).to_string())),
        ("display_digest", json!(display(0x55).to_string())),
        ("schema_digest", json!(schema(0x44).to_string())),
        (
            "intent_digest",
            json!(format!("sha256:{}", "00".repeat(32))),
        ),
    ] {
        let mut tampered = value.clone();
        tampered[field] = replacement;
        assert!(
            serde_json::from_value::<PermissionIntent>(tampered).is_err(),
            "{field}"
        );
    }
}

#[test]
fn workflow_causation_tampering_is_rejected() {
    let value = serde_json::to_value(intent()).unwrap();

    let mut removed = value.clone();
    removed.as_object_mut().unwrap().remove("workflow");
    assert!(serde_json::from_value::<PermissionIntent>(removed).is_err());

    for (field, replacement) in [
        ("run_id", "wrun_other"),
        ("node_id", "wnode_other"),
        ("attempt_id", "watt_other"),
    ] {
        let mut changed = value.clone();
        changed["workflow"][field] = json!(replacement);
        assert!(serde_json::from_value::<PermissionIntent>(changed).is_err());
    }

    let direct = PermissionIntent::bind(
        PermissionIntentSeed::new("Bash", action(1), display(3), schema(2), None).unwrap(),
        7,
        9,
        "interaction-1",
    )
    .unwrap();
    let mut added = serde_json::to_value(direct).unwrap();
    added["workflow"] = serde_json::to_value(workflow()).unwrap();
    assert!(serde_json::from_value::<PermissionIntent>(added).is_err());
}

#[test]
fn malformed_seed_and_binding_fail_closed() {
    assert!(PermissionIntentSeed::new("", action(1), display(3), schema(2), None).is_err());
    assert!(
        PermissionIntentSeed::new("bad\nname", action(1), display(3), schema(2), None).is_err()
    );
    let invalid_workflow = WorkflowPermissionCausation {
        run_id: WorkflowRunId::from("../escape"),
        node_id: WorkflowNodeId::from("wnode_1"),
        attempt_id: WorkflowAttemptId::from("watt_1"),
    };
    assert!(
        PermissionIntentSeed::new(
            "Bash",
            action(1),
            display(3),
            schema(2),
            Some(invalid_workflow),
        )
        .is_err()
    );
    let seed = PermissionIntentSeed::new("Bash", action(1), display(3), schema(2), None).unwrap();
    assert!(PermissionIntent::bind(seed.clone(), 0, 1, "token").is_err());
    assert!(PermissionIntent::bind(seed.clone(), 1, 0, "token").is_err());
    assert!(PermissionIntent::bind(seed, 1, 1, "").is_err());
}

#[test]
fn unknown_fields_and_unsupported_versions_are_rejected() {
    let mut value = serde_json::to_value(intent()).unwrap();
    value["unknown"] = Value::Bool(true);
    assert!(serde_json::from_value::<PermissionIntent>(value).is_err());

    let mut seed = serde_json::to_value(intent().seed()).unwrap();
    seed["version"] = json!(1);
    assert!(serde_json::from_value::<PermissionIntentSeed>(seed).is_err());
}
