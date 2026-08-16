use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use loopal_vault_api::{AuditError, AuditMetadata, AuditResult, AuditSink, ProtectedOp, VaultOp};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOp {
    Resolved,
    Redacted,
}

pub struct JsonlAuditSink {
    dir: PathBuf,
}

impl JsonlAuditSink {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn try_new(dir: PathBuf) -> AuditResult<Self> {
        fs::create_dir_all(&dir).map_err(|source| AuditError::CreateDirectory {
            path: dir.clone(),
            source,
        })?;
        let path = dir.join("secret_access.jsonl");
        open_secure(&path)?
            .sync_data()
            .map_err(|source| AuditError::Sync { path, source })?;
        Ok(Self { dir })
    }

    pub fn record_runtime(
        &self,
        op: RuntimeOp,
        names: &[String],
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        for name in names {
            self.append(SerializedOp::Runtime(op), "post_effect", name, metadata)?;
        }
        Ok(())
    }

    fn append(
        &self,
        op: SerializedOp,
        phase: &'static str,
        name: &str,
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        self.append_with(op, phase, name, metadata, open_secure)
    }

    fn append_with<W: AuditWriter>(
        &self,
        op: SerializedOp,
        phase: &'static str,
        name: &str,
        metadata: &AuditMetadata<'_>,
        open: impl FnOnce(&Path) -> AuditResult<W>,
    ) -> AuditResult<()> {
        fs::create_dir_all(&self.dir).map_err(|source| AuditError::CreateDirectory {
            path: self.dir.clone(),
            source,
        })?;
        let path = self.dir.join("secret_access.jsonl");
        let mut line = serde_json::to_vec(&AuditEntry {
            ts: chrono::Utc::now().to_rfc3339(),
            op,
            phase,
            name,
            session_id: metadata.session_id,
            cwd: metadata.cwd,
            agent_name: metadata.agent_name,
            depth: metadata.depth,
            connection_generation: metadata.connection_generation,
            tool_name: metadata.tool_name,
            tool_call_id: metadata.tool_call_id,
            action_digest: metadata.action_digest,
            schema_digest: metadata.schema_digest,
            intent_digest: metadata.intent_digest,
            workflow_run_id: metadata.workflow_run_id,
            workflow_node_id: metadata.workflow_node_id,
            workflow_attempt_id: metadata.workflow_attempt_id,
            workflow_phase: metadata.workflow_phase,
            decision: metadata.decision,
            decision_source: metadata.decision_source,
            spawn_target: metadata.spawn_target,
            model: metadata.model,
            permission_mode: metadata.permission_mode,
            decision_mode: metadata.decision_mode,
            sandbox_policy: metadata.sandbox_policy,
            pid: std::process::id(),
        })
        .map_err(|error| AuditError::Serialization(error.to_string()))?;
        line.push(b'\n');
        let mut file = open(&path)?;
        file.write_all(&line).map_err(|source| AuditError::Write {
            path: path.clone(),
            source,
        })?;
        file.flush().map_err(|source| AuditError::Flush {
            path: path.clone(),
            source,
        })?;
        sync_writer(&file).map_err(|source| AuditError::Sync { path, source })
    }
}

impl AuditSink for JsonlAuditSink {
    fn record(&self, op: VaultOp, key: &str, metadata: &AuditMetadata<'_>) -> AuditResult<()> {
        self.append(SerializedOp::Vault(op), "pre_effect", key, metadata)
    }

    fn record_protected(
        &self,
        op: ProtectedOp,
        subject: &str,
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        self.append(SerializedOp::Protected(op), "pre_effect", subject, metadata)
    }
}

trait AuditWriter: Write {
    fn sync_data(&self) -> io::Result<()>;
}

impl AuditWriter for File {
    fn sync_data(&self) -> io::Result<()> {
        File::sync_data(self)
    }
}

fn sync_writer(writer: &impl AuditWriter) -> io::Result<()> {
    writer.sync_data()
}

fn open_secure(path: &Path) -> AuditResult<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(|source| AuditError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    ensure_secure_mode(path, &file)?;
    Ok(file)
}

#[cfg(unix)]
fn ensure_secure_mode(path: &Path, file: &File) -> AuditResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = file.metadata().map_err(|source| AuditError::Permissions {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.permissions().mode() & 0o777 == 0o600 {
        return Ok(());
    }
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o600);
    file.set_permissions(permissions)
        .map_err(|source| AuditError::Permissions {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn ensure_secure_mode(_path: &Path, _file: &File) -> AuditResult<()> {
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(untagged)]
enum SerializedOp {
    Vault(VaultOp),
    Runtime(RuntimeOp),
    Protected(ProtectedOp),
}

include!("audit_entry.rs");

pub fn default_telemetry_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".loopal").join("telemetry"))
}

#[cfg(test)]
mod tests;
