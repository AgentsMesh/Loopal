//! Test harness for single-`AgeVault` tests. Shared between `store_test`
//! and `store_edge_test`.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::{TempDir, tempdir};

use loopal_vault_age::{AgeVault, DiscoveredIdentity, Recipients};

use crate::ssh_fixtures as fx;

pub const PUBKEY_ALICE: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";

#[cfg(unix)]
pub fn write_key(path: &Path, content: &str, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content).unwrap();
    let mut p = fs::metadata(path).unwrap().permissions();
    p.set_mode(mode);
    fs::set_permissions(path, p).unwrap();
}

#[cfg(not(unix))]
pub fn write_key(path: &Path, content: &str, _mode: u32) {
    fs::write(path, content).unwrap();
}

pub struct Harness {
    pub _dir: TempDir,
    pub store: AgeVault,
    pub vault: std::path::PathBuf,
    pub recipients: std::path::PathBuf,
    pub identity: Arc<DiscoveredIdentity>,
}

pub fn build_harness() -> Harness {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    write_key(&key_path, fx::ED25519_UNENCRYPTED, 0o600);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());

    let recipients_path = dir.path().join(".age-recipients");
    let mut rec = Recipients::new();
    rec.add_line(PUBKEY_ALICE).unwrap();
    rec.write(&recipients_path).unwrap();

    let vault_path = dir.path().join("secrets.yaml.age");
    let store = AgeVault::new(
        vault_path.clone(),
        recipients_path.clone(),
        identity.clone(),
    );
    Harness {
        _dir: dir,
        store,
        vault: vault_path,
        recipients: recipients_path,
        identity,
    }
}
