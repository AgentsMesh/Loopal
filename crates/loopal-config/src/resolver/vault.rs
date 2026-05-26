use std::path::PathBuf;
use std::sync::Arc;

use loopal_secret_runtime::{JsonlAuditSink, MergedVault, default_telemetry_dir};
use loopal_vault_age::AgeVault;
use loopal_vault_api::Vault;

pub(super) fn build_secret_store(
    vaults_dir: Option<PathBuf>,
    default_vault_name: Option<&str>,
) -> Option<Arc<dyn Vault>> {
    let dir = vaults_dir?;
    let default_name = default_vault_name.unwrap_or("default");

    let all_names = loopal_vault_age::list_initialized_vaults(&dir);
    if all_names.is_empty() {
        return None;
    }

    let identity = match loopal_vault_age::discover() {
        Ok(i) => Arc::new(i),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "vaults present but SSH identity discovery failed; vaults disabled"
            );
            return None;
        }
    };

    let audit: Arc<dyn loopal_vault_api::AuditSink> = match default_telemetry_dir() {
        Some(td) => Arc::new(JsonlAuditSink::new(td)),
        None => Arc::new(loopal_vault_api::NoopAuditSink),
    };

    let mk = |name: &str| -> Arc<dyn Vault> {
        let store = dir.join(format!("{name}.vault")).join("store.age");
        let recipients = dir.join(format!("{name}.vault")).join("recipients");
        Arc::new(AgeVault::with_audit(
            store,
            recipients,
            identity.clone(),
            audit.clone(),
        ))
    };

    let default = if all_names.iter().any(|n| n == default_name) {
        (default_name.to_string(), mk(default_name))
    } else if default_vault_name.is_some() {
        // Explicit default_vault config pointing at a missing vault must
        // fail loudly — silent fallback would mask the misconfiguration.
        tracing::error!(
            requested = default_name,
            available = ?all_names,
            "configured default_vault not found; vault subsystem disabled"
        );
        return None;
    } else {
        let first = all_names[0].clone();
        if first != "default" {
            tracing::info!(
                using = first.as_str(),
                "no 'default' vault present; using first alphabetical as default"
            );
        }
        (first.clone(), mk(&first))
    };
    let others: Vec<(String, Arc<dyn Vault>)> = all_names
        .iter()
        .filter(|n| n.as_str() != default.0)
        .map(|n| (n.clone(), mk(n)))
        .collect();

    if others.is_empty() {
        Some(default.1)
    } else {
        Some(Arc::new(MergedVault::new(default, others)))
    }
}
