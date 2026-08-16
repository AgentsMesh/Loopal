use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::*;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Vault(VaultOp, String, Option<String>, Option<String>),
    Protected(ProtectedOp, String, Option<u64>, Option<String>),
}

#[derive(Default)]
struct RecordingSink {
    calls: Mutex<Vec<Call>>,
}

impl AuditSink for RecordingSink {
    fn record(&self, op: VaultOp, key: &str, metadata: &AuditMetadata<'_>) -> AuditResult<()> {
        self.calls.lock().unwrap().push(Call::Vault(
            op,
            key.into(),
            metadata.session_id.map(str::to_string),
            metadata.cwd.map(|path| path.display().to_string()),
        ));
        Ok(())
    }

    fn record_protected(
        &self,
        op: ProtectedOp,
        subject: &str,
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        self.calls.lock().unwrap().push(Call::Protected(
            op,
            subject.into(),
            metadata.connection_generation,
            metadata.action_digest.map(str::to_string),
        ));
        Ok(())
    }
}

#[test]
fn noop_sink_is_an_explicit_successful_opt_out() {
    let sink = NoopAuditSink;
    sink.record(VaultOp::Decrypted, "key", &AuditMetadata::default())
        .unwrap();
    sink.record_protected(ProtectedOp::ToolEffect, "call", &AuditMetadata::default())
        .unwrap();
}

#[test]
fn arc_forwards_both_operation_families_with_metadata() {
    let concrete = Arc::new(RecordingSink::default());
    let sink: Arc<dyn AuditSink> = concrete.clone();
    let cwd = Path::new("/project");
    let metadata = AuditMetadata {
        session_id: Some("session"),
        cwd: Some(cwd),
        agent_name: Some("root"),
        depth: Some(2),
        connection_generation: Some(7),
        tool_name: Some("Bash"),
        tool_call_id: Some("call"),
        action_digest: Some("action"),
        schema_digest: Some("schema"),
        ..AuditMetadata::default()
    };
    sink.record(VaultOp::Encrypted, "key", &metadata).unwrap();
    sink.record_protected(ProtectedOp::ToolEffect, "call", &metadata)
        .unwrap();

    assert_eq!(
        *concrete.calls.lock().unwrap(),
        [
            Call::Vault(
                VaultOp::Encrypted,
                "key".into(),
                Some("session".into()),
                Some("/project".into())
            ),
            Call::Protected(
                ProtectedOp::ToolEffect,
                "call".into(),
                Some(7),
                Some("action".into())
            )
        ]
    );
}

#[test]
fn audit_error_variants_include_path_and_source() {
    let path = PathBuf::from("/audit");
    let variants = [
        AuditError::CreateDirectory {
            path: path.clone(),
            source: std::io::Error::other("create"),
        },
        AuditError::Open {
            path: path.clone(),
            source: std::io::Error::other("open"),
        },
        AuditError::Permissions {
            path: path.clone(),
            source: std::io::Error::other("permissions"),
        },
        AuditError::Write {
            path: path.clone(),
            source: std::io::Error::other("write"),
        },
        AuditError::Flush {
            path: path.clone(),
            source: std::io::Error::other("flush"),
        },
        AuditError::Sync {
            path,
            source: std::io::Error::other("sync"),
        },
    ];
    for error in variants {
        assert!(error.to_string().contains("/audit"));
    }
    assert!(
        AuditError::Serialization("json".into())
            .to_string()
            .contains("json")
    );
}

#[test]
fn operation_enums_serialize_as_snake_case() {
    assert_eq!(
        serde_json::to_string(&ProtectedOp::PermissionDecision).unwrap(),
        "\"permission_decision\""
    );
    assert_eq!(
        serde_json::to_string(&ProtectedOp::SpawnAuthority).unwrap(),
        "\"spawn_authority\""
    );
    assert_eq!(
        serde_json::to_string(&ProtectedOp::ToolEffect).unwrap(),
        "\"tool_effect\""
    );
    assert_eq!(
        serde_json::to_string(&ProtectedOp::WorkflowAttemptLifecycle).unwrap(),
        "\"workflow_attempt_lifecycle\""
    );
    assert_eq!(
        serde_json::to_string(&VaultOp::Decrypted).unwrap(),
        "\"decrypted\""
    );
    assert_eq!(
        serde_json::to_string(&VaultOp::Encrypted).unwrap(),
        "\"encrypted\""
    );
    assert_eq!(
        serde_json::to_string(&VaultOp::Rekeyed).unwrap(),
        "\"rekeyed\""
    );
}
