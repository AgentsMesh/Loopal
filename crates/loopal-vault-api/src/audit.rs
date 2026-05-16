use std::sync::Arc;

use serde::Serialize;

/// Operations a vault implementation reports for auditing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultOp {
    /// Plaintext was decrypted out (e.g. `get` returned a value).
    Decrypted,
    /// Ciphertext was rewritten (e.g. `put` / `delete` / `rekey`).
    Encrypted,
    /// Vault re-encrypted with current recipient set.
    Rekeyed,
}

/// Audit destination injected into a vault. Vault impls call `record` after
/// every operation; consumers decide where audit lands (jsonl, syslog, ...).
pub trait AuditSink: Send + Sync {
    fn record(&self, op: VaultOp, key: &str, session_id: Option<&str>);
}

/// No-op sink. Use when audit isn't needed (tests, ad-hoc CLI).
pub struct NoopAuditSink;

impl AuditSink for NoopAuditSink {
    fn record(&self, _op: VaultOp, _key: &str, _session_id: Option<&str>) {}
}

impl<T: AuditSink + ?Sized> AuditSink for Arc<T> {
    fn record(&self, op: VaultOp, key: &str, session_id: Option<&str>) {
        (**self).record(op, key, session_id);
    }
}
