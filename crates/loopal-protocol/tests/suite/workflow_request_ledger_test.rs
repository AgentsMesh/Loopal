use loopal_protocol::*;

#[test]
fn request_ledger_replays_exact_payload_and_rejects_mismatch() {
    let mut ledger = WorkflowRequestLedger::default();
    let record = WorkflowRequestRecord {
        request_id: "wreq_start".into(),
        operation: "start".into(),
        payload: serde_json::json!({"spec": "same"}),
        response: serde_json::json!({"run_id": "wrun_one"}),
    };
    ledger.record(record.clone()).unwrap();
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(
        ledger
            .decide(&record.request_id, &record.operation, &record.payload)
            .unwrap(),
        WorkflowRequestDecision::Replay(&record.response)
    );
    ledger.record(record.clone()).unwrap();
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        ledger.decide(
            &record.request_id,
            &record.operation,
            &serde_json::json!({"spec": "different"}),
        ),
        Err(WorkflowRequestError::PayloadMismatch { .. })
    ));
}

#[test]
fn invalid_request_ids_and_operation_fail_before_effect() {
    let ledger = WorkflowRequestLedger::default();
    assert_eq!(
        ledger.decide(
            &WorkflowRequestId::new("../bad"),
            "start",
            &serde_json::Value::Null,
        ),
        Err(WorkflowRequestError::InvalidRequestId)
    );
    for operation in [
        String::new(),
        "x".repeat(MAX_WORKFLOW_REQUEST_OPERATION_BYTES + 1),
    ] {
        assert_eq!(
            ledger.decide(
                &WorkflowRequestId::new("wreq_operation"),
                &operation,
                &serde_json::Value::Null,
            ),
            Err(WorkflowRequestError::InvalidOperation)
        );
    }
}

#[test]
fn oversized_payload_is_rejected_by_decide_before_effect() {
    let ledger = WorkflowRequestLedger::default();
    let payload = serde_json::json!("x".repeat(MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES + 1));
    assert_eq!(
        ledger.decide(&WorkflowRequestId::new("wreq_large"), "start", &payload),
        Err(WorkflowRequestError::PayloadTooLarge)
    );
}

#[test]
fn ledger_reservation_rejects_max_response_before_effect_and_replay_at_full_works() {
    let mut ledger = WorkflowRequestLedger::default();
    let response = serde_json::json!("x".repeat(MAX_WORKFLOW_REQUEST_RESPONSE_BYTES - 1_024));
    let mut index = 0;
    loop {
        let record = WorkflowRequestRecord {
            request_id: WorkflowRequestId::new(format!("wreq_bytes_{index}")),
            operation: "start".into(),
            payload: serde_json::Value::Null,
            response: response.clone(),
        };
        if ledger.record(record.clone()) == Err(WorkflowRequestError::LedgerFull) {
            assert_eq!(
                ledger.decide_with_response_size(
                    &record.request_id,
                    &record.operation,
                    &record.payload,
                    MAX_WORKFLOW_REQUEST_RESPONSE_BYTES,
                ),
                Err(WorkflowRequestError::LedgerFull)
            );
            let first = &ledger.records()[0];
            assert!(matches!(
                ledger.decide(&first.request_id, &first.operation, &first.payload),
                Ok(WorkflowRequestDecision::Replay(_))
            ));
            break;
        }
        index += 1;
    }
}

#[test]
fn full_record_count_is_rejected_by_decide_before_effect() {
    let mut ledger = WorkflowRequestLedger::default();
    for index in 0..MAX_WORKFLOW_REQUEST_RECORDS {
        ledger
            .record(WorkflowRequestRecord {
                request_id: WorkflowRequestId::new(format!("wreq_{index}")),
                operation: "start".into(),
                payload: serde_json::Value::Null,
                response: serde_json::Value::Null,
            })
            .unwrap();
    }
    assert_eq!(
        ledger.decide(
            &WorkflowRequestId::new("wreq_overflow"),
            "start",
            &serde_json::Value::Null,
        ),
        Err(WorkflowRequestError::LedgerFull)
    );
}
