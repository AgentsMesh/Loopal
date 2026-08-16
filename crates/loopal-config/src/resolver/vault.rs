use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopal_error::ConfigError;
use loopal_secret_runtime::{JsonlAuditSink, MergedVault};
use loopal_vault_age::AgeVault;
use loopal_vault_api::{AuditSink, Vault};

pub(super) fn build_secret_store(
    vaults_dir: Option<PathBuf>,
    default_vault_name: Option<&str>,
    audit_dir: PathBuf,
) -> Result<Option<Arc<dyn Vault>>, ConfigError> {
    build_secret_store_with(
        vaults_dir,
        default_vault_name,
        audit_dir,
        loopal_vault_age::list_initialized_vaults,
        |dir| {
            JsonlAuditSink::try_new(dir)
                .map(|sink| Arc::new(sink) as Arc<dyn AuditSink>)
                .map_err(|error| ConfigError::InvalidValue {
                    field: "telemetry.telemetry_dir".into(),
                    reason: format!("protected vault audit unavailable: {error}"),
                })
        },
        || loopal_vault_age::discover().map(Arc::new),
        make_age_vault,
    )
}

fn make_age_vault(
    dir: &Path,
    name: &str,
    identity: &Arc<loopal_vault_age::DiscoveredIdentity>,
    audit: Arc<dyn AuditSink>,
) -> Arc<dyn Vault> {
    let store = dir.join(format!("{name}.vault")).join("store.age");
    let recipients = dir.join(format!("{name}.vault")).join("recipients");
    Arc::new(AgeVault::with_audit(
        store,
        recipients,
        identity.clone(),
        audit,
    ))
}

fn build_secret_store_with<I, E>(
    vaults_dir: Option<PathBuf>,
    default_vault_name: Option<&str>,
    audit_dir: PathBuf,
    list_vaults: impl FnOnce(&Path) -> Vec<String>,
    open_audit: impl FnOnce(PathBuf) -> Result<Arc<dyn AuditSink>, ConfigError>,
    discover_identity: impl FnOnce() -> Result<I, E>,
    make_vault: impl Fn(&Path, &str, &I, Arc<dyn AuditSink>) -> Arc<dyn Vault>,
) -> Result<Option<Arc<dyn Vault>>, ConfigError>
where
    E: Display,
{
    let Some(dir) = vaults_dir else {
        return Ok(None);
    };
    let Some(selection) = select_vaults(list_vaults(&dir), default_vault_name)? else {
        return Ok(None);
    };

    if default_vault_name.is_none() && selection.default != "default" {
        tracing::info!(
            using = selection.default.as_str(),
            "no 'default' vault present; using first alphabetical as default"
        );
    }
    let audit = open_audit(audit_dir)?;
    let identity = match discover_identity() {
        Ok(identity) => identity,
        Err(error) => {
            tracing::warn!(
                %error,
                "vaults present but SSH identity discovery failed; vaults disabled"
            );
            return Ok(None);
        }
    };
    let default = (
        selection.default.clone(),
        make_vault(&dir, &selection.default, &identity, audit.clone()),
    );
    let others: Vec<_> = selection
        .others
        .into_iter()
        .map(|name| {
            let vault = make_vault(&dir, &name, &identity, audit.clone());
            (name, vault)
        })
        .collect();

    Ok(if others.is_empty() {
        Some(default.1)
    } else {
        Some(Arc::new(MergedVault::new(default, others)))
    })
}

struct VaultSelection {
    default: String,
    others: Vec<String>,
}

fn select_vaults(
    all_names: Vec<String>,
    requested: Option<&str>,
) -> Result<Option<VaultSelection>, ConfigError> {
    if all_names.is_empty() {
        return Ok(None);
    }
    let default = match requested {
        Some(name) if all_names.iter().any(|available| available == name) => name.to_string(),
        Some(name) => {
            return Err(ConfigError::InvalidValue {
                field: "secrets.default_vault".into(),
                reason: format!(
                    "vault '{name}' is not initialized; available vaults: {}",
                    all_names.join(", ")
                ),
            });
        }
        None if all_names.iter().any(|name| name == "default") => "default".into(),
        None => all_names[0].clone(),
    };
    let others = all_names
        .into_iter()
        .filter(|name| name != &default)
        .collect();
    Ok(Some(VaultSelection { default, others }))
}

#[cfg(test)]
mod tests;
