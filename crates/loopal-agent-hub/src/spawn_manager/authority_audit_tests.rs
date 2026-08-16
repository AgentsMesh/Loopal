use std::sync::Arc;

use loopal_vault_api::NoopAuditSink;
use serde_json::{Value, json};

use super::authority_audit::SpawnAudit;
use super::spawn_audit_test_support::{Sink, agent_fixture, workflow_causation};

fn derived_params() -> Value {
    json!({
        "model": "test-model",
        "permission_mode": "ask_any_write",
        "decision_mode": "manual",
        "sandbox_policy": "read_only",
        "depth": 1,
    })
}

fn cross_hub_error(
    hub: &crate::Hub,
    execution: &crate::types::AgentExecutionRef,
    params: &Value,
) -> String {
    SpawnAudit::for_cross_hub(hub, "child", "destination", execution, params)
        .err()
        .unwrap()
}

#[tokio::test]
async fn prepared_and_cross_hub_require_runtime_authority() {
    let (hub, prepared, execution) = agent_fixture(Some(Arc::new(NoopAuditSink))).await;
    assert!(hub.lock().await.registry.unregister_exact(&execution));

    let locked = hub.lock().await;
    assert_eq!(
        SpawnAudit::for_prepared(&locked, &prepared).err().unwrap(),
        "spawn requester runtime authority is unavailable"
    );
    assert_eq!(
        cross_hub_error(&locked, &execution, &derived_params()),
        "spawn requester runtime authority is unavailable"
    );
}

#[tokio::test]
async fn cross_hub_rejects_missing_or_invalid_authority_fields() {
    let (hub, _prepared, execution) = agent_fixture(Some(Arc::new(NoopAuditSink))).await;
    let locked = hub.lock().await;

    for (field, value, expected) in [
        (
            "model",
            json!(null),
            "derived spawn 'model' must be a string",
        ),
        (
            "permission_mode",
            json!("invalid"),
            "invalid permission mode 'invalid'",
        ),
        (
            "decision_mode",
            json!("invalid"),
            "invalid decision mode 'invalid'",
        ),
        (
            "sandbox_policy",
            json!("invalid"),
            "invalid sandbox policy 'invalid'",
        ),
    ] {
        let mut params = derived_params();
        params[field] = value;
        let error = cross_hub_error(&locked, &execution, &params);
        assert!(error.contains(expected), "field {field}: {error}");
    }
}

#[tokio::test]
async fn cross_hub_rejects_non_u32_depth_and_missing_sink() {
    let (hub, _prepared, execution) = agent_fixture(Some(Arc::new(NoopAuditSink))).await;
    let locked = hub.lock().await;
    for depth in [json!(null), json!(-1), json!(u64::from(u32::MAX) + 1)] {
        let mut params = derived_params();
        params["depth"] = depth;
        assert_eq!(
            cross_hub_error(&locked, &execution, &params),
            "invalid derived spawn depth"
        );
    }
    drop(locked);

    let (hub, _prepared, execution) = agent_fixture(None).await;
    let locked = hub.lock().await;
    assert_eq!(
        cross_hub_error(&locked, &execution, &derived_params()),
        "protected audit unavailable"
    );
}

#[tokio::test]
async fn prepared_workflow_causation_overrides_requester_causation() {
    let sink = Arc::new(Sink::new(false));
    let (hub, mut prepared, _execution) =
        agent_fixture(Some(sink.clone() as Arc<dyn loopal_vault_api::AuditSink>)).await;
    prepared.workflow_permission_causation = Some(workflow_causation());

    {
        let locked = hub.lock().await;
        SpawnAudit::for_prepared(&locked, &prepared)
            .unwrap()
            .append()
            .await
            .unwrap();
    }

    let records = sink.records();
    assert_eq!(records[0].workflow_run_id.as_deref(), Some("wrun_prepared"));
    assert_eq!(
        records[0].workflow_node_id.as_deref(),
        Some("wnode_prepared")
    );
    assert_eq!(
        records[0].workflow_attempt_id.as_deref(),
        Some("watt_prepared")
    );
}
