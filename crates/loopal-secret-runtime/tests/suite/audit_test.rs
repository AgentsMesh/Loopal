use loopal_secret_runtime::{JsonlAuditSink, RuntimeOp};
use loopal_vault_api::{AuditSink, VaultOp};
use tempfile::tempdir;

#[test]
fn vault_record_creates_jsonl_file() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record(VaultOp::Decrypted, "openai_key", None);
    let path = dir.path().join("secret_access.jsonl");
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert_eq!(lines.len(), 1);
}

#[test]
fn entry_contains_name_op_and_pid_but_not_value() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_runtime(RuntimeOp::Resolved, &["openai_key".into()], Some("sess-1"));
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let line = content.lines().next().unwrap();
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["name"], "openai_key");
    assert_eq!(v["op"], "resolved");
    assert_eq!(v["session_id"], "sess-1");
    assert!(v["pid"].is_number());
    assert!(v["ts"].as_str().unwrap().contains('T'));
    assert!(v.get("value").is_none());
    assert!(v.get("secret").is_none());
    assert!(v.get("plaintext").is_none());
}

#[test]
fn session_id_field_omitted_when_none() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record(VaultOp::Decrypted, "k", None);
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let v: serde_json::Value = serde_json::from_str(content.lines().next().unwrap()).unwrap();
    assert!(v.get("session_id").is_none());
}

#[test]
fn multiple_records_accumulate() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_runtime(RuntimeOp::Resolved, &["a".into()], None);
    sink.record_runtime(RuntimeOp::Redacted, &["b".into()], None);
    sink.record(VaultOp::Encrypted, "c", None);
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    assert_eq!(content.lines().count(), 3);
}

#[test]
fn runtime_batch_writes_one_line_per_name() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    let names = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    sink.record_runtime(RuntimeOp::Resolved, &names, None);
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
fn jsonl_file_mode_is_0600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record_runtime(RuntimeOp::Resolved, &["k".into()], None);
    let path = dir.path().join("secret_access.jsonl");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn op_serializes_as_snake_case() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::new(dir.path().to_path_buf());
    sink.record(VaultOp::Decrypted, "a", None);
    sink.record(VaultOp::Encrypted, "b", None);
    sink.record(VaultOp::Rekeyed, "", None);
    let content = std::fs::read_to_string(dir.path().join("secret_access.jsonl")).unwrap();
    let lines: Vec<_> = content.lines().collect();
    let v1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let v2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let v3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(v1["op"], "decrypted");
    assert_eq!(v2["op"], "encrypted");
    assert_eq!(v3["op"], "rekeyed");
}
