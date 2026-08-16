use loopal_protocol::*;

use crate::workflow_support::*;

#[test]
fn opaque_ids_have_distinct_prefixes_and_string_wire_shape() {
    let ids = [
        (WorkflowRunId::generate().to_string(), "wrun_"),
        (WorkflowNodeId::generate().to_string(), "wnode_"),
        (WorkflowAttemptId::generate().to_string(), "watt_"),
        (WorkflowRequestId::generate().to_string(), "wreq_"),
    ];
    for (id, prefix) in ids {
        assert!(id.starts_with(prefix));
        assert!(id.len() <= MAX_WORKFLOW_ID_BYTES);
        assert_eq!(serde_json::to_value(&id).unwrap(), serde_json::json!(id));
    }
    let encoded = serde_json::to_value(WorkflowRunId::new("wrun_fixed")).unwrap();
    assert_eq!(encoded, serde_json::json!("wrun_fixed"));
}

#[test]
fn command_dtos_roundtrip_without_concrete_hub_fields() {
    let request = WorkflowStartRequest {
        request_id: "wreq_start".into(),
        spec: text_spec(),
    };
    let value = serde_json::to_value(&request).unwrap();
    let json = serde_json::to_string(&value).unwrap();
    assert!(!json.contains("generation"));
    assert!(!json.contains("sandbox"));
    assert!(!json.contains("permission"));
    assert!(!json.contains("cwd"));
    assert_eq!(
        serde_json::from_value::<WorkflowStartRequest>(value).unwrap(),
        request
    );
}

#[test]
fn start_lookup_has_a_strict_stable_wire_shape() {
    let request = WorkflowStartLookupRequest {
        request_id: WorkflowRequestId::new("human_00112233445566778899aabbccddeeff"),
    };
    let request_value = serde_json::json!({
        "request_id": "human_00112233445566778899aabbccddeeff"
    });
    assert_eq!(serde_json::to_value(&request).unwrap(), request_value);
    assert_eq!(
        serde_json::from_value::<WorkflowStartLookupRequest>(request_value).unwrap(),
        request
    );

    let response = WorkflowStartResponse {
        summary: WorkflowRunSummary::from(&planned(text_spec())),
    };
    for (lookup, expected) in [
        (
            WorkflowStartLookupResponse::NotFound,
            serde_json::json!({"status": "not_found"}),
        ),
        (
            WorkflowStartLookupResponse::Found {
                response: response.clone(),
            },
            serde_json::json!({
                "status": "found",
                "response": {
                    "summary": {
                        "id": "wrun_test",
                        "run_goal": "complete the workflow",
                        "state": "planned",
                        "revision": 0,
                        "output_node": "output",
                        "counts": {
                            "pending": 2,
                            "ready": 0,
                            "active": 0,
                            "succeeded": 0,
                            "failed": 0,
                            "cancelled": 0,
                            "skipped": 0
                        },
                        "created_at_unix_ms": 100,
                        "updated_at_unix_ms": 100
                    }
                }
            }),
        ),
        (
            WorkflowStartLookupResponse::Conflict,
            serde_json::json!({"status": "conflict"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&lookup).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<WorkflowStartLookupResponse>(expected).unwrap(),
            lookup
        );
    }

    assert!(
        serde_json::from_value::<WorkflowStartLookupRequest>(serde_json::json!({
            "request_id": "human_00112233445566778899aabbccddeeff",
            "owner": "forged"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WorkflowStartLookupResponse>(serde_json::json!({
            "status": "not_found",
            "response": null
        }))
        .is_err()
    );
}

#[test]
fn worker_handshake_is_strict_and_keeps_execution_identity_transport_bound() {
    let request = WorkflowWorkerHandshakeRequest {
        causation: WorkflowPermissionCausation {
            run_id: WorkflowRunId::new("wrun_handshake"),
            node_id: WorkflowNodeId::new("wnode_handshake"),
            attempt_id: WorkflowAttemptId::new("watt_handshake"),
        },
        capability: WorkflowAttemptCapability::parse("11".repeat(32)).unwrap(),
    };
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(
        serde_json::from_value::<WorkflowWorkerHandshakeRequest>(value.clone()).unwrap(),
        request
    );
    assert!(value.get("execution").is_none());
    assert!(value.get("connection_generation").is_none());

    let mut unknown = value;
    unknown
        .as_object_mut()
        .unwrap()
        .insert("execution".into(), serde_json::json!({"agent": "forged"}));
    assert!(serde_json::from_value::<WorkflowWorkerHandshakeRequest>(unknown).is_err());
}

#[test]
fn summary_counts_all_node_states() {
    let mut run = planned(text_spec());
    run.state = WorkflowRunState::Running;
    run.nodes[0].state = WorkflowNodeState::Running;
    run.nodes[1].state = WorkflowNodeState::Skipped;
    let summary = WorkflowRunSummary::from(&run);
    assert_eq!(summary.counts.active, 1);
    assert_eq!(summary.counts.skipped, 1);
    assert_eq!(summary.counts.pending, 0);
}

#[test]
fn workflow_run_changed_preserves_existing_untagged_event_convention() {
    let summary = WorkflowRunSummary::from(&planned(text_spec()));
    let payload = AgentEventPayload::WorkflowRunChanged(summary.clone());
    let value = serde_json::to_value(&payload).unwrap();
    assert!(value.get("WorkflowRunChanged").is_some());
    let decoded: AgentEventPayload = serde_json::from_value(value).unwrap();
    let AgentEventPayload::WorkflowRunChanged(decoded) = decoded else {
        panic!("wrong event variant")
    };
    assert_eq!(decoded, summary);
}

#[test]
fn workflow_public_dtos_do_not_expose_connection_generation() {
    let source = include_str!("../../src/workflow/state.rs");
    let event = include_str!("../../src/workflow/event.rs");
    let command = include_str!("../../src/workflow/command.rs");
    for content in [source, event, command] {
        assert!(!content.contains("connection_generation"));
        assert!(!content.contains("AgentExecutionRef"));
    }
}
