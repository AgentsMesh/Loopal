use loopal_protocol::{
    PermissionActionDigest, PermissionDisplayDigest, PermissionIntent, PermissionIntentSeed,
    PermissionReceipt, PermissionReceiptError, PermissionSchemaDigest, WorkflowAttemptId,
    WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
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

fn causation() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::from("wrun_receipt"),
        node_id: WorkflowNodeId::from("wnode_receipt"),
        attempt_id: WorkflowAttemptId::from("watt_receipt"),
    }
}

fn seed() -> PermissionIntentSeed {
    PermissionIntentSeed::new(
        "Bash",
        action(0x11),
        display(0x22),
        schema(0x33),
        Some(causation()),
    )
    .unwrap()
}

fn intent() -> PermissionIntent {
    PermissionIntent::bind(seed(), 7, 9, "interaction-receipt").unwrap()
}

fn receipt_value() -> Value {
    serde_json::to_value(PermissionReceipt::issue(&intent(), "audit-receipt").unwrap()).unwrap()
}

fn decode(value: Value) -> PermissionReceipt {
    serde_json::from_value(value).unwrap()
}

#[test]
fn issued_and_local_receipts_expose_and_validate_every_binding() {
    let intent = intent();
    let receipt = PermissionReceipt::issue_for_intent(&intent, "audit-receipt").unwrap();
    assert_eq!(receipt.action_digest(), action(0x11));
    assert_eq!(receipt.schema_digest(), schema(0x33));
    assert_eq!(receipt.intent_digest(), intent.intent_digest());
    assert_eq!(receipt.execution_generation(), 7);
    assert_eq!(receipt.ui_generation(), 9);
    assert_eq!(receipt.interaction_token(), "interaction-receipt");
    assert_eq!(receipt.workflow(), Some(&causation()));
    assert_eq!(receipt.audit_issuance(), "audit-receipt");
    receipt.validate_for(intent.seed()).unwrap();
    receipt
        .validate_effect_binding(action(0x11), schema(0x33), 7, Some(&causation()))
        .unwrap();

    let local = PermissionReceipt::issue_local(intent.seed(), "local-policy").unwrap();
    assert_eq!(local.execution_generation(), 1);
    assert_eq!(local.ui_generation(), 1);
    assert!(local.interaction_token().starts_with("local:"));
    local.validate_for(intent.seed()).unwrap();
}

#[test]
fn receipt_binding_checks_reject_each_independent_mismatch() {
    let expected = seed();
    for (field, replacement) in [
        (
            "action_digest",
            json!(PermissionActionDigest::from_bytes([0x44; 32]).to_string()),
        ),
        (
            "schema_digest",
            json!(PermissionSchemaDigest::from_bytes([0x55; 32]).to_string()),
        ),
        (
            "workflow",
            json!({
                "run_id": "wrun_other",
                "node_id": "wnode_receipt",
                "attempt_id": "watt_receipt",
            }),
        ),
        (
            "intent_digest",
            json!(format!("sha256:{}", "66".repeat(32))),
        ),
    ] {
        let mut value = receipt_value();
        value[field] = replacement;
        assert_eq!(
            decode(value).validate_for(&expected),
            Err(PermissionReceiptError::Binding),
            "{field}"
        );
    }

    let receipt = decode(receipt_value());
    for (actual_action, actual_schema, generation, workflow) in [
        (action(0x44), schema(0x33), 7, Some(causation())),
        (action(0x11), schema(0x44), 7, Some(causation())),
        (action(0x11), schema(0x33), 8, Some(causation())),
        (action(0x11), schema(0x33), 7, None),
    ] {
        assert_eq!(
            receipt.validate_effect_binding(
                actual_action,
                actual_schema,
                generation,
                workflow.as_ref(),
            ),
            Err(PermissionReceiptError::Binding)
        );
    }
}

#[test]
fn malformed_receipt_fields_fail_closed() {
    for (field, replacement, expected) in [
        (
            "execution_generation",
            json!(0),
            PermissionReceiptError::Generation,
        ),
        (
            "ui_generation",
            json!(0),
            PermissionReceiptError::Generation,
        ),
        (
            "interaction_token",
            json!(""),
            PermissionReceiptError::Token,
        ),
        (
            "interaction_token",
            json!("x".repeat(129)),
            PermissionReceiptError::Token,
        ),
        (
            "interaction_token",
            json!("bad\ntoken"),
            PermissionReceiptError::Token,
        ),
        ("audit_issuance", json!(""), PermissionReceiptError::Token),
        (
            "audit_issuance",
            json!("x".repeat(129)),
            PermissionReceiptError::Token,
        ),
        (
            "audit_issuance",
            json!("bad\naudit"),
            PermissionReceiptError::Token,
        ),
        (
            "workflow",
            json!({
                "run_id": "../escape",
                "node_id": "wnode_receipt",
                "attempt_id": "watt_receipt",
            }),
            PermissionReceiptError::Binding,
        ),
    ] {
        let mut value = receipt_value();
        value[field] = replacement;
        assert_eq!(
            decode(value).validate_for(&seed()),
            Err(expected),
            "{field}"
        );
    }

    assert_eq!(
        PermissionReceipt::issue(&intent(), ""),
        Err(PermissionReceiptError::Token)
    );
}
