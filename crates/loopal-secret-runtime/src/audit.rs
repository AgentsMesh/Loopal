use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use loopal_vault_api::{AuditSink, VaultOp};
use serde::Serialize;

/// Runtime-side operations (resolver/redactor) audited alongside vault ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOp {
    /// `<secret_ref:NAME>` replaced with plaintext in a tool argument.
    Resolved,
    /// Known plaintext scrubbed from tool output before LLM sees it.
    Redacted,
}

/// JSONL audit sink writing to `<dir>/secret_access.jsonl` with mode 0600.
///
/// Implements `loopal_vault_api::AuditSink` so an `AgeVault` can be
/// constructed with this sink and have its operations land in the same log.
pub struct JsonlAuditSink {
    dir: PathBuf,
}

impl JsonlAuditSink {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn record_runtime(&self, op: RuntimeOp, names: &[String], session_id: Option<&str>) {
        for name in names {
            let _ = self.append(SerializedOp::Runtime(op), name, session_id);
        }
    }

    fn append(
        &self,
        op: SerializedOp,
        name: &str,
        session_id: Option<&str>,
    ) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join("secret_access.jsonl");
        let entry = AuditEntry {
            ts: chrono::Utc::now().to_rfc3339(),
            op,
            name,
            session_id,
            pid: std::process::id(),
        };
        let line = serde_json::to_string(&entry)
            .map_err(|e| std::io::Error::other(format!("audit serialize: {e}")))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{line}")?;
        write_secure_mode(&path)?;
        Ok(())
    }
}

impl AuditSink for JsonlAuditSink {
    fn record(&self, op: VaultOp, key: &str, session_id: Option<&str>) {
        let _ = self.append(SerializedOp::Vault(op), key, session_id);
    }
}

#[cfg(unix)]
fn write_secure_mode(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut p = fs::metadata(path)?.permissions();
    p.set_mode(0o600);
    fs::set_permissions(path, p)
}

#[cfg(not(unix))]
fn write_secure_mode(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(untagged)]
enum SerializedOp {
    Vault(VaultOp),
    Runtime(RuntimeOp),
}

#[derive(Serialize)]
struct AuditEntry<'a> {
    ts: String,
    op: SerializedOp,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    pid: u32,
}

/// Default telemetry directory: `~/.loopal/telemetry/`.
pub fn default_telemetry_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".loopal").join("telemetry"))
}
