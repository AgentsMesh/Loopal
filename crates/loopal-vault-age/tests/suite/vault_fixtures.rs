//! Shared test fixtures for vault-age e2e tests.
//! Builds real on-disk `.loopal/vaults/<name>.vault/store.age` files using
//! the unencrypted ed25519 SSH key fixture.

use std::path::Path;
use std::sync::Arc;

use loopal_vault_age::{AgeVault, DiscoveredIdentity, Recipients};
use loopal_vault_api::Vault;
use tempfile::{TempDir, tempdir};

use crate::ssh_fixtures as fx;

pub const PUBKEY_ALICE: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";

#[cfg(unix)]
pub fn write_key(path: &Path, content: &str, mode: u32) {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content).unwrap();
    let mut p = fs::metadata(path).unwrap().permissions();
    p.set_mode(mode);
    fs::set_permissions(path, p).unwrap();
}

#[cfg(not(unix))]
pub fn write_key(path: &Path, content: &str, _mode: u32) {
    std::fs::write(path, content).unwrap();
}

/// Build a project skeleton: tempdir + identity + N initialized vaults.
pub struct Skel {
    pub _tmp: TempDir,
    pub vaults_dir: std::path::PathBuf,
    pub identity: Arc<DiscoveredIdentity>,
}

impl Skel {
    pub fn new() -> Self {
        let tmp = tempdir().unwrap();
        let cwd = tmp.path().to_path_buf();
        let key_path = cwd.join("id_ed25519");
        write_key(&key_path, fx::ED25519_UNENCRYPTED, 0o600);
        let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
        let vaults_dir = cwd.join(".loopal").join("vaults");
        std::fs::create_dir_all(&vaults_dir).unwrap();
        Self {
            _tmp: tmp,
            vaults_dir,
            identity,
        }
    }

    /// Create an initialized `<name>.vault/` with the given pubkey as recipient.
    pub async fn init_vault(&self, name: &str, pubkey: &str) -> AgeVault {
        let dir = self.vaults_dir.join(format!("{name}.vault"));
        std::fs::create_dir_all(&dir).unwrap();
        let rec = dir.join("recipients");
        let mut r = Recipients::new();
        r.add_line(pubkey).unwrap();
        r.write(&rec).unwrap();
        let store = dir.join("store.age");
        let vault = AgeVault::new(store, rec, self.identity.clone());
        vault.rekey().await.unwrap();
        let dir = self.vaults_dir.join(format!("{name}.vault"));
        AgeVault::new(
            dir.join("store.age"),
            dir.join("recipients"),
            self.identity.clone(),
        )
    }
}
