use std::sync::Arc;

use loopal_secret_client::{HUB_RPC_BUDGET, SecretClient, SecretError};
use loopal_vault_api::AuditMetadata;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use tracing::warn;

use crate::audit::{JsonlAuditSink, RuntimeOp};
use crate::redactor::Redactor;
use crate::resolver::{collect_wire_refs, resolve_in_value};

mod audit;

use audit::{record_audit, record_redaction_hits_inner};
pub use audit::{record_redaction_hits, record_redaction_hits_with_audit};

pub async fn apply_resolver(
    tool_name: &str,
    effective_input: &mut Value,
    whitelist: &[&str],
    client: Option<&Arc<dyn SecretClient>>,
    session_id: &str,
) -> Vec<(String, SecretString)> {
    apply_resolver_inner(
        tool_name,
        effective_input,
        whitelist,
        client,
        session_id,
        None,
    )
    .await
}

pub async fn apply_resolver_with_audit(
    tool_name: &str,
    effective_input: &mut Value,
    whitelist: &[&str],
    client: Option<&Arc<dyn SecretClient>>,
    session_id: &str,
    audit: &JsonlAuditSink,
) -> Vec<(String, SecretString)> {
    apply_resolver_inner(
        tool_name,
        effective_input,
        whitelist,
        client,
        session_id,
        Some(audit),
    )
    .await
}

async fn apply_resolver_inner(
    tool_name: &str,
    effective_input: &mut Value,
    whitelist: &[&str],
    client: Option<&Arc<dyn SecretClient>>,
    session_id: &str,
    audit: Option<&JsonlAuditSink>,
) -> Vec<(String, SecretString)> {
    let Some(client) = client else {
        return Vec::new();
    };
    if whitelist.is_empty() {
        return Vec::new();
    }
    let names = collect_wire_refs(effective_input, whitelist);
    if names.is_empty() {
        return Vec::new();
    }
    let mut resolved: std::collections::HashMap<String, SecretString> =
        std::collections::HashMap::new();
    let mut seed: Vec<(String, SecretString)> = Vec::new();
    let mut hard_failure = false;
    for n in &names {
        match client.get(n, HUB_RPC_BUDGET).await {
            Ok(v) => {
                resolved.insert(n.clone(), v.clone());
                seed.push((n.clone(), v));
            }
            Err(SecretError::SecretNotFound(_)) => {}
            Err(e) => {
                hard_failure = true;
                warn!(
                    tool = tool_name,
                    name = %n,
                    error = %e,
                    "secret_ref resolve failed (non-NotFound); preserving wire refs so the \
                     protected-effect boundary fails closed"
                );
            }
        }
    }
    if hard_failure {
        return Vec::new();
    }
    let report = resolve_in_value(effective_input, &resolved, whitelist);
    if !report.missing.is_empty() {
        warn!(tool = tool_name, missing = ?report.missing, "secret refs missing");
    }
    record_audit(
        RuntimeOp::Resolved,
        &report.resolved_names,
        &AuditMetadata {
            session_id: Some(session_id),
            ..AuditMetadata::default()
        },
        audit,
    );

    let leaked = detect_argv_exposure(effective_input, &seed);
    if !leaked.is_empty() {
        warn!(
            tool = tool_name,
            secrets = ?leaked,
            "secret plaintext substituted into shell-argv-visible field (e.g. `command`); \
             prefer the `env` field for secret injection"
        );
        record_audit(
            RuntimeOp::Resolved,
            &leaked,
            &AuditMetadata {
                session_id: Some(session_id),
                ..AuditMetadata::default()
            },
            audit,
        );
    }
    seed
}

/// Detect which secrets were substituted into argv-visible fields
/// (currently just `command`). Pure function — caller decides whether to
/// warn or audit. Returned names are deduped.
///
/// "argv-visible" means: the field becomes part of a child process's argv,
/// which is observable to `ps` and similar tools. The supported channel
/// for shell tools is the `env` field instead.
pub fn detect_argv_exposure(
    effective_input: &Value,
    seed: &[(String, SecretString)],
) -> Vec<String> {
    if seed.is_empty() {
        return Vec::new();
    }
    const ARGV_FIELDS: &[&str] = &["command"];
    let mut leaked: Vec<String> = Vec::new();
    for field in ARGV_FIELDS {
        let Some(cmd) = effective_input.get(*field).and_then(|v| v.as_str()) else {
            continue;
        };
        for (name, secret) in seed {
            if cmd.contains(secret.expose_secret()) {
                leaked.push(name.clone());
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    leaked.retain(|n| seen.insert(n.clone()));
    leaked
}

/// Scan tool output for known plaintext values and replace with placeholders.
pub fn apply_redactor(
    tool_name: &str,
    content: String,
    seed: &[(String, SecretString)],
    session_id: &str,
) -> String {
    apply_redactor_inner(tool_name, content, seed, session_id, None)
}

pub fn apply_redactor_with_audit(
    tool_name: &str,
    content: String,
    seed: &[(String, SecretString)],
    session_id: &str,
    audit: &JsonlAuditSink,
) -> String {
    apply_redactor_inner(tool_name, content, seed, session_id, Some(audit))
}

fn apply_redactor_inner(
    tool_name: &str,
    content: String,
    seed: &[(String, SecretString)],
    session_id: &str,
    audit: Option<&JsonlAuditSink>,
) -> String {
    if seed.is_empty() {
        return content;
    }
    let redactor = Redactor::from_pairs(seed);
    let (redacted, hit_names) = redactor.scan_and_redact(&content);
    record_redaction_hits_inner(tool_name, &hit_names, session_id, audit);
    redacted
}
