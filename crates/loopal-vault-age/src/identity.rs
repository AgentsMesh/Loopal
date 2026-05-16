use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use loopal_vault_api::{VaultError, VaultResult};

const PREFERRED_KEYS: &[&str] = &["id_ed25519", "id_rsa"];

pub struct DiscoveredIdentity {
    pub path: PathBuf,
    pub identity: age::ssh::Identity,
}

impl std::fmt::Debug for DiscoveredIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoveredIdentity")
            .field("path", &self.path)
            .field("encrypted", &self.is_encrypted())
            .finish()
    }
}

impl DiscoveredIdentity {
    pub fn is_encrypted(&self) -> bool {
        matches!(self.identity, age::ssh::Identity::Encrypted(_))
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self.identity, age::ssh::Identity::Unsupported(_))
    }

    /// Returns Err when the identity cannot be used for non-interactive vault
    /// operations — e.g. passphrase-protected key with no ssh-agent socket.
    pub fn ensure_usable(&self) -> VaultResult<()> {
        if self.is_encrypted() {
            return Err(VaultError::PassphraseProtected {
                path: self.path.clone(),
            });
        }
        Ok(())
    }
}

pub fn discover() -> VaultResult<DiscoveredIdentity> {
    let home = dirs::home_dir().ok_or(VaultError::IdentityMissing)?;
    discover_in(&home.join(".ssh"))
}

pub fn discover_in(ssh_dir: &Path) -> VaultResult<DiscoveredIdentity> {
    for name in PREFERRED_KEYS {
        let candidate = ssh_dir.join(name);
        if candidate.exists() {
            return load(&candidate);
        }
    }
    Err(VaultError::IdentityMissing)
}

pub fn load(path: &Path) -> VaultResult<DiscoveredIdentity> {
    check_permissions(path)?;
    let file = fs::File::open(path).map_err(|e| VaultError::Backend(format!("open: {e}")))?;
    let reader = BufReader::new(file);
    let filename = path.to_string_lossy().to_string();
    let identity = age::ssh::Identity::from_buffer(reader, Some(filename))
        .map_err(|e| VaultError::DecryptionFailed(format!("parse {}: {e}", path.display())))?;
    Ok(DiscoveredIdentity {
        path: path.to_path_buf(),
        identity,
    })
}

#[cfg(unix)]
fn check_permissions(path: &Path) -> VaultResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta = fs::metadata(path).map_err(|e| VaultError::Backend(format!("stat: {e}")))?;
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(VaultError::InsecureIdentityPermissions(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_permissions(_path: &Path) -> VaultResult<()> {
    Ok(())
}
