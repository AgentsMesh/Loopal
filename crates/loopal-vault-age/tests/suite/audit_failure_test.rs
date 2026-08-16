use std::sync::Arc;

use loopal_vault_age::AgeVault;
use loopal_vault_api::{
    AuditError, AuditMetadata, AuditResult, AuditSink, ProtectedOp, Vault, VaultError, VaultOp,
};
use secrecy::SecretString;

use crate::store_fixtures::{Harness, build_harness};

struct FailingAuditSink;

impl AuditSink for FailingAuditSink {
    fn record(&self, _op: VaultOp, _key: &str, _metadata: &AuditMetadata<'_>) -> AuditResult<()> {
        Err(AuditError::Serialization("forced failure".into()))
    }

    fn record_protected(
        &self,
        _op: ProtectedOp,
        _subject: &str,
        _metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        Err(AuditError::Serialization("forced failure".into()))
    }
}

fn audited_store(harness: &Harness) -> AgeVault {
    AgeVault::with_audit(
        harness.vault.clone(),
        harness.recipients.clone(),
        harness.identity.clone(),
        Arc::new(FailingAuditSink),
    )
}

fn assert_audit_error(error: VaultError) {
    assert!(matches!(error, VaultError::Audit(_)), "got: {error}");
}

#[tokio::test]
async fn audit_failure_precedes_decryption() {
    let harness = build_harness();
    std::fs::write(&harness.vault, b"invalid age ciphertext").unwrap();
    let store = audited_store(&harness);

    let error = store
        .get_audited("api_key", &AuditMetadata::default())
        .await
        .unwrap_err();

    assert_audit_error(error);
}

#[tokio::test]
async fn audit_failure_prevents_protected_writes() {
    let harness = build_harness();
    harness
        .store
        .put("existing", SecretString::from("original"))
        .await
        .unwrap();
    let before = std::fs::read(&harness.vault).unwrap();
    let store = audited_store(&harness);

    let put_error = store
        .put("new_key", SecretString::from("new value"))
        .await
        .unwrap_err();
    assert_audit_error(put_error);
    assert_eq!(std::fs::read(&harness.vault).unwrap(), before);

    let delete_error = store.delete("existing").await.unwrap_err();
    assert_audit_error(delete_error);
    assert_eq!(std::fs::read(&harness.vault).unwrap(), before);

    let rekey_error = store.rekey().await.unwrap_err();
    assert_audit_error(rekey_error);
    assert_eq!(std::fs::read(&harness.vault).unwrap(), before);
}
