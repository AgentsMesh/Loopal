use loopal_vault_api::AuditMetadata;
use tracing::warn;

use crate::audit::{JsonlAuditSink, RuntimeOp, default_telemetry_dir};

pub fn record_redaction_hits(tool_name: &str, hit_names: &[String], session_id: &str) {
    record_redaction_hits_inner(tool_name, hit_names, session_id, None);
}

pub fn record_redaction_hits_with_audit(
    tool_name: &str,
    hit_names: &[String],
    session_id: &str,
    audit: &JsonlAuditSink,
) {
    record_redaction_hits_inner(tool_name, hit_names, session_id, Some(audit));
}

pub(super) fn record_redaction_hits_inner(
    tool_name: &str,
    hit_names: &[String],
    session_id: &str,
    audit: Option<&JsonlAuditSink>,
) {
    if hit_names.is_empty() {
        return;
    }
    warn!(tool = tool_name, hit = ?hit_names, "redacted plaintext from tool output");
    record_audit(
        RuntimeOp::Redacted,
        hit_names,
        &AuditMetadata {
            session_id: Some(session_id),
            ..AuditMetadata::default()
        },
        audit,
    );
}

pub(super) fn record_audit(
    op: RuntimeOp,
    names: &[String],
    metadata: &AuditMetadata<'_>,
    audit: Option<&JsonlAuditSink>,
) {
    let default_sink;
    let sink = match audit {
        Some(sink) => sink,
        None => {
            let Some(dir) = default_telemetry_dir() else {
                warn!("runtime audit directory unavailable");
                return;
            };
            default_sink = JsonlAuditSink::new(dir);
            &default_sink
        }
    };
    if let Err(error) = sink.record_runtime(op, names, metadata) {
        warn!(%error, "runtime protected audit failed");
    }
}
