use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopal_vault_api::{VaultError, VaultResult};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::{AgeVault, DiscoveredIdentity};

pub const DEFAULT_VAULT_NAME: &str = "default";

static NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z][a-z0-9_-]*$").expect("vault name regex"));

pub fn validate_vault_name(name: &str) -> VaultResult<()> {
    if NAME_RE.is_match(name) {
        Ok(())
    } else {
        Err(VaultError::InvalidVaultName(name.to_string()))
    }
}

pub(crate) fn vaults_dir(cwd: &Path) -> PathBuf {
    cwd.join(".loopal").join("vaults")
}

pub(crate) fn vault_dir(cwd: &Path, name: &str) -> PathBuf {
    vaults_dir(cwd).join(format!("{name}.vault"))
}

pub(crate) fn store_path(cwd: &Path, name: &str) -> PathBuf {
    vault_dir(cwd, name).join("store.age")
}

pub(crate) fn recipients_path_named(cwd: &Path, name: &str) -> PathBuf {
    vault_dir(cwd, name).join("recipients")
}

/// Enumerate vault names with initialized `store.age` files.
/// Delegates to `discovery::list_initialized_vaults` so CLI and runtime
/// see the same set.
pub(crate) fn enumerate_vault_names(cwd: &Path) -> Vec<String> {
    crate::discovery::list_initialized_vaults(&vaults_dir(cwd))
}

pub(crate) fn discover_identity_or_exit() -> Option<DiscoveredIdentity> {
    let identity = match crate::discover() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("SSH identity not found: {e}");
            eprintln!("Generate one with: ssh-keygen -t ed25519");
            return None;
        }
    };
    if let Some(msg) = crate::passphrase_warning(identity.is_encrypted()) {
        eprintln!("warning: {msg}");
    }
    Some(identity)
}

pub(crate) fn open_vault_or_exit(cwd: &Path, name: &str) -> Option<AgeVault> {
    if let Err(e) = validate_vault_name(name) {
        eprintln!("{e}");
        return None;
    }
    let store = store_path(cwd, name);
    if !store.exists() {
        eprintln!(
            "no vault {name:?} at {}; run `loopal vaults init{}` first",
            store.display(),
            if name == DEFAULT_VAULT_NAME {
                String::new()
            } else {
                format!(" {name}")
            }
        );
        return None;
    }
    let identity = discover_identity_or_exit()?;
    Some(AgeVault::new(
        store,
        recipients_path_named(cwd, name),
        Arc::new(identity),
    ))
}

pub(crate) fn read_ssh_public_key_alongside(private_key_path: &Path) -> std::io::Result<String> {
    let mut p = private_key_path.as_os_str().to_owned();
    p.push(".pub");
    std::fs::read_to_string(std::path::PathBuf::from(p)).map(|s| s.trim().to_string())
}
