use loopal_secret_runtime::JsonlAuditSink;
use loopal_vault_api::{AuditMetadata, AuditSink, ProtectedOp};

#[test]
fn protected_operations_serialize_identity_and_policy_only() {
    let dir = tempfile::tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_protected(
        ProtectedOp::PermissionDecision,
        "tool-call-1",
        &AuditMetadata {
            session_id: Some("session-1"),
            agent_name: Some("main"),
            connection_generation: Some(7),
            tool_name: Some("Bash"),
            tool_call_id: Some("tool-call-1"),
            action_digest: Some("sha256:action"),
            schema_digest: Some("sha256:schema"),
            intent_digest: Some("sha256:intent"),
            workflow_run_id: Some("wrun_1"),
            workflow_node_id: Some("node_1"),
            workflow_attempt_id: Some("attempt_1"),
            workflow_phase: Some("activate"),
            decision: Some("allow"),
            decision_source: Some("manual"),
            ..AuditMetadata::default()
        },
    )
    .unwrap();
    sink.record_protected(
        ProtectedOp::SpawnAuthority,
        "child",
        &AuditMetadata {
            session_id: Some("session-1"),
            agent_name: Some("main"),
            depth: Some(1),
            connection_generation: Some(7),
            spawn_target: Some("local"),
            model: Some("worker-model"),
            permission_mode: Some("ask_any_write"),
            decision_mode: Some("manual"),
            sandbox_policy: Some("read_only"),
            ..AuditMetadata::default()
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    for forbidden in ["action_input", "prompt", "secret", "plaintext"] {
        assert!(!content.contains(forbidden), "found {forbidden}");
    }
    let records = content
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records[0]["op"], "permission_decision");
    assert_eq!(records[0]["phase"], "pre_effect");
    assert_eq!(records[0]["workflow_run_id"], "wrun_1");
    assert_eq!(records[0]["workflow_node_id"], "node_1");
    assert_eq!(records[0]["workflow_attempt_id"], "attempt_1");
    assert_eq!(records[0]["workflow_phase"], "activate");
    assert_eq!(records[0]["intent_digest"], "sha256:intent");
    assert_eq!(records[0]["decision"], "allow");
    assert_eq!(records[0]["decision_source"], "manual");
    assert_eq!(records[1]["op"], "spawn_authority");
    assert_eq!(records[1]["spawn_target"], "local");
    assert_eq!(records[1]["model"], "worker-model");
    assert_eq!(records[1]["permission_mode"], "ask_any_write");
    assert_eq!(records[1]["decision_mode"], "manual");
    assert_eq!(records[1]["sandbox_policy"], "read_only");
}

#[test]
fn absent_protected_metadata_is_omitted() {
    let dir = tempfile::tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_protected(
        ProtectedOp::PermissionDecision,
        "call",
        &AuditMetadata::default(),
    )
    .unwrap();

    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let value: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    for field in [
        "workflow_run_id",
        "workflow_node_id",
        "workflow_attempt_id",
        "workflow_phase",
        "decision",
        "decision_source",
        "spawn_target",
        "model",
        "permission_mode",
        "decision_mode",
        "sandbox_policy",
    ] {
        assert!(value.get(field).is_none(), "unexpected {field}");
    }
}
