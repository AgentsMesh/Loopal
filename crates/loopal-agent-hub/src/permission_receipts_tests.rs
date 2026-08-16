use loopal_protocol::{
    PermissionActionDigest, PermissionDisplayDigest, PermissionIntent, PermissionIntentDigest,
    PermissionIntentSeed, PermissionSchemaDigest, WorkflowAttemptId, WorkflowNodeId,
    WorkflowPermissionCausation, WorkflowRunId,
};

use super::*;

fn workflow() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_receipt"),
        node_id: WorkflowNodeId::new("wnode_receipt"),
        attempt_id: WorkflowAttemptId::new("watt_receipt"),
    }
}

fn intent(token: &str) -> PermissionIntent {
    PermissionIntent::bind(
        PermissionIntentSeed::new(
            "Bash",
            PermissionActionDigest::from_bytes([1; 32]),
            PermissionDisplayDigest::from_bytes([2; 32]),
            PermissionSchemaDigest::from_bytes([3; 32]),
            Some(workflow()),
        )
        .unwrap(),
        7,
        9,
        token,
    )
    .unwrap()
}

fn assert_binding_mismatch(mutate: impl FnOnce(&mut Issuance), current_ui_generation: u64) {
    let execution = AgentExecutionRef::local("worker", 7);
    let intent = intent("receipt-token");
    let mut registry = PermissionReceiptRegistry::default();
    let receipt = registry.issue(&intent, &execution, true).unwrap();
    mutate(
        registry
            .issuances
            .get_mut(receipt.audit_issuance())
            .unwrap(),
    );

    assert_eq!(
        registry.consume(
            &receipt,
            intent.seed().action_digest(),
            intent.seed().schema_digest(),
            &execution,
            intent.seed().workflow(),
            current_ui_generation,
        ),
        Err("permission receipt issuance binding mismatch".into())
    );
}

#[test]
fn ledger_rejects_each_independent_issuance_binding_mismatch() {
    assert_binding_mismatch(
        |issuance| issuance.action_digest = PermissionActionDigest::from_bytes([4; 32]),
        9,
    );
    assert_binding_mismatch(
        |issuance| issuance.schema_digest = PermissionSchemaDigest::from_bytes([4; 32]),
        9,
    );
    assert_binding_mismatch(
        |issuance| issuance.intent_digest = PermissionIntentDigest::from_bytes([4; 32]),
        9,
    );
    assert_binding_mismatch(
        |issuance| issuance.execution = AgentExecutionRef::local("other", 7),
        9,
    );
    assert_binding_mismatch(|issuance| issuance.execution_generation = 8, 9);
    assert_binding_mismatch(|issuance| issuance.ui_generation = 8, 9);
    assert_binding_mismatch(|issuance| issuance.workflow = None, 9);
    assert_binding_mismatch(|_| {}, 10);
}

#[test]
fn revoking_an_execution_preserves_other_issuances() {
    let first = AgentExecutionRef::local("first", 7);
    let second = AgentExecutionRef::local("second", 7);
    let first_intent = intent("first-token");
    let second_intent = intent("second-token");
    let mut registry = PermissionReceiptRegistry::default();
    let first_receipt = registry.issue(&first_intent, &first, false).unwrap();
    let second_receipt = registry.issue(&second_intent, &second, false).unwrap();

    registry.revoke_execution(&first);

    assert!(
        !registry
            .issuances
            .contains_key(first_receipt.audit_issuance())
    );
    assert!(
        registry
            .issuances
            .contains_key(second_receipt.audit_issuance())
    );
    assert_eq!(registry.len(), 1);
}
