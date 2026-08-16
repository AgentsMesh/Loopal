use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

pub type AuditResult<T> = Result<T, AuditError>;

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("audit serialization failed: {0}")]
    Serialization(String),
    #[error("audit directory creation failed at {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("audit file open failed at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("audit file permissions failed at {path}: {source}")]
    Permissions {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("audit write failed at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("audit flush failed at {path}: {source}")]
    Flush {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("audit sync failed at {path}: {source}")]
    Sync {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AuditMetadata<'a> {
    pub session_id: Option<&'a str>,
    pub cwd: Option<&'a Path>,
    pub agent_name: Option<&'a str>,
    pub depth: Option<u32>,
    pub connection_generation: Option<u64>,
    pub tool_name: Option<&'a str>,
    pub tool_call_id: Option<&'a str>,
    pub action_digest: Option<&'a str>,
    pub schema_digest: Option<&'a str>,
    pub intent_digest: Option<&'a str>,
    pub workflow_run_id: Option<&'a str>,
    pub workflow_node_id: Option<&'a str>,
    pub workflow_attempt_id: Option<&'a str>,
    pub workflow_phase: Option<&'a str>,
    pub decision: Option<&'a str>,
    pub decision_source: Option<&'a str>,
    pub spawn_target: Option<&'a str>,
    pub model: Option<&'a str>,
    pub permission_mode: Option<&'a str>,
    pub decision_mode: Option<&'a str>,
    pub sandbox_policy: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedOp {
    PermissionDecision,
    SpawnAuthority,
    ToolEffect,
    WorkflowAttemptLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultOp {
    Decrypted,
    Encrypted,
    Rekeyed,
}

pub trait AuditSink: Send + Sync {
    fn record(&self, op: VaultOp, key: &str, metadata: &AuditMetadata<'_>) -> AuditResult<()>;
    fn record_protected(
        &self,
        op: ProtectedOp,
        subject: &str,
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()>;
}

/// Explicit opt-out for tests and ad-hoc commands.
pub struct NoopAuditSink;

impl AuditSink for NoopAuditSink {
    fn record(&self, _op: VaultOp, _key: &str, _metadata: &AuditMetadata<'_>) -> AuditResult<()> {
        Ok(())
    }

    fn record_protected(
        &self,
        _op: ProtectedOp,
        _subject: &str,
        _metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        Ok(())
    }
}

impl<T: AuditSink + ?Sized> AuditSink for Arc<T> {
    fn record(&self, op: VaultOp, key: &str, metadata: &AuditMetadata<'_>) -> AuditResult<()> {
        (**self).record(op, key, metadata)
    }

    fn record_protected(
        &self,
        op: ProtectedOp,
        subject: &str,
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        (**self).record_protected(op, subject, metadata)
    }
}

#[cfg(test)]
mod tests;
