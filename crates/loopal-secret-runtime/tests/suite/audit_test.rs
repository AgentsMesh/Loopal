use loopal_secret_runtime::{JsonlAuditSink, RuntimeOp};
use loopal_vault_api::{AuditMetadata, AuditSink, ProtectedOp, VaultOp};
use std::path::Path;

use tempfile::tempdir;

#[test]
fn vault_record_creates_jsonl_file() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record(VaultOp::Decrypted, "openai_key", &Default::default())
        .unwrap();
    let path = dir.path().join("secret_access.jsonl");
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert_eq!(lines.len(), 1);
}

#[test]
fn entry_contains_authenticated_fields_but_not_plaintext() {
    let dir = tempdir().unwrap();
    let cwd = Path::new("/project");
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_runtime(
        RuntimeOp::Resolved,
        &["openai_key".into()],
        &AuditMetadata {
            session_id: Some("sess-1"),
            cwd: Some(cwd),
            agent_name: Some("root-agent"),
            depth: Some(2),
            tool_name: Some("Bash"),
            ..AuditMetadata::default()
        },
    )
    .unwrap();
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let value: serde_json::Value = serde_json::from_str(content.lines().next().unwrap()).unwrap();
    assert_eq!(value["name"], "openai_key");
    assert_eq!(value["op"], "resolved");
    assert_eq!(value["phase"], "post_effect");
    assert_eq!(value["session_id"], "sess-1");
    assert_eq!(value["cwd"], "/project");
    assert_eq!(value["agent_name"], "root-agent");
    assert_eq!(value["depth"], 2);
    assert_eq!(value["tool_name"], "Bash");
    assert!(value.get("connection_generation").is_none());
    assert!(value.get("tool_call_id").is_none());
    assert!(value.get("action_digest").is_none());
    assert!(value.get("schema_digest").is_none());
    assert!(value.get("intent_digest").is_none());
    assert!(value["pid"].is_number());
    assert!(value["ts"].as_str().unwrap().contains('T'));
    assert!(value.get("value").is_none());
    assert!(value.get("secret").is_none());
    assert!(value.get("plaintext").is_none());
}

#[test]
fn session_id_field_omitted_when_none() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record(VaultOp::Decrypted, "k", &Default::default())
        .unwrap();
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let v: serde_json::Value = serde_json::from_str(content.lines().next().unwrap()).unwrap();
    assert!(v.get("session_id").is_none());
}

#[test]
fn multiple_records_accumulate() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_runtime(RuntimeOp::Resolved, &["a".into()], &Default::default())
        .unwrap();
    sink.record_runtime(RuntimeOp::Redacted, &["b".into()], &Default::default())
        .unwrap();
    sink.record(VaultOp::Encrypted, "c", &Default::default())
        .unwrap();
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    assert_eq!(content.lines().count(), 3);
}

#[test]
fn runtime_batch_writes_one_line_per_name() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    let names = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    sink.record_runtime(RuntimeOp::Resolved, &names, &Default::default())
        .unwrap();
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert_eq!(lines.len(), 3);
    for (i, expected) in names.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(lines[i]).unwrap();
        assert_eq!(v["name"], *expected);
    }
}

#[cfg(unix)]
#[test]
fn jsonl_file_mode_is_created_and_repaired_to_0600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_runtime(RuntimeOp::Resolved, &["k".into()], &Default::default())
        .unwrap();
    let path = dir.path().join("secret_access.jsonl");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    sink.record(VaultOp::Encrypted, "k", &Default::default())
        .unwrap();
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn record_runtime_propagates_storage_errors() {
    let dir = tempdir().unwrap();
    let blocker = dir.path().join("not-a-directory");
    std::fs::write(&blocker, "occupied").unwrap();
    let sink = JsonlAuditSink::new(blocker);

    let error = sink
        .record_runtime(RuntimeOp::Resolved, &["k".into()], &Default::default())
        .unwrap_err();

    assert!(error.to_string().contains("directory creation failed"));
}

#[test]
fn protected_effect_records_authenticated_digests_without_input() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_protected(
        ProtectedOp::ToolEffect,
        "tool-call-1",
        &AuditMetadata {
            session_id: Some("session-1"),
            cwd: Some(Path::new("/project")),
            agent_name: Some("main"),
            depth: Some(0),
            connection_generation: Some(7),
            tool_name: Some("Bash"),
            tool_call_id: Some("tool-call-1"),
            action_digest: Some("sha256:action"),
            schema_digest: Some("sha256:schema"),
            ..AuditMetadata::default()
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    assert!(!content.contains("action_input"));
    assert!(!content.contains("plaintext"));
    let value: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(value["op"], "tool_effect");
    assert_eq!(value["phase"], "pre_effect");
    assert_eq!(value["connection_generation"], 7);
    assert_eq!(value["action_digest"], "sha256:action");
    assert_eq!(value["schema_digest"], "sha256:schema");
}

#[test]
fn op_serializes_as_snake_case() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record(VaultOp::Decrypted, "a", &Default::default())
        .unwrap();
    sink.record(VaultOp::Encrypted, "b", &Default::default())
        .unwrap();
    sink.record(VaultOp::Rekeyed, "", &Default::default())
        .unwrap();
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let lines: Vec<_> = content.lines().collect();
    let v1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let v2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let v3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(v1["op"], "decrypted");
    assert_eq!(v2["op"], "encrypted");
    assert_eq!(v3["op"], "rekeyed");
    assert_eq!(v2["phase"], "pre_effect");
}
