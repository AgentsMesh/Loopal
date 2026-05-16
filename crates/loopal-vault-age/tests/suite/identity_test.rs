use std::fs;
use std::path::Path;
use tempfile::tempdir;

use loopal_vault_age::{discover_in, load};
use loopal_vault_api::VaultError;

use crate::ssh_fixtures as fx;

#[cfg(unix)]
fn write_key(path: &Path, content: &str, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content).unwrap();
    let mut perm = fs::metadata(path).unwrap().permissions();
    perm.set_mode(mode);
    fs::set_permissions(path, perm).unwrap();
}

#[cfg(not(unix))]
fn write_key(path: &Path, content: &str, _mode: u32) {
    fs::write(path, content).unwrap();
}

#[test]
fn discover_prefers_ed25519_over_rsa() {
    let dir = tempdir().unwrap();
    write_key(
        &dir.path().join("id_ed25519"),
        fx::ED25519_UNENCRYPTED,
        0o600,
    );
    write_key(&dir.path().join("id_rsa"), fx::RSA_PRIVATE, 0o600);

    let found = discover_in(dir.path()).unwrap();
    assert!(found.path.ends_with("id_ed25519"));
    assert!(!found.is_encrypted());
    assert!(found.is_supported());
}

#[test]
fn discover_falls_back_to_rsa() {
    let dir = tempdir().unwrap();
    write_key(&dir.path().join("id_rsa"), fx::RSA_PRIVATE, 0o600);

    let found = discover_in(dir.path()).unwrap();
    assert!(found.path.ends_with("id_rsa"));
    assert!(!found.is_encrypted());
}

#[test]
fn discover_returns_identity_missing_when_no_keys() {
    let dir = tempdir().unwrap();
    match discover_in(dir.path()) {
        Err(VaultError::IdentityMissing) => {}
        other => panic!("expected IdentityMissing, got {other:?}"),
    }
}

#[test]
fn load_ed25519_unencrypted() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("id_ed25519");
    write_key(&path, fx::ED25519_UNENCRYPTED, 0o600);

    let found = load(&path).unwrap();
    assert!(!found.is_encrypted());
    assert!(found.is_supported());
}

#[test]
fn load_ed25519_encrypted_reports_encrypted() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("id_ed25519");
    write_key(&path, fx::ED25519_AES256_CBC, 0o600);

    let found = load(&path).unwrap();
    assert!(found.is_encrypted());
}

#[test]
fn load_malformed_returns_decryption_failed() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("id_garbage");
    write_key(&path, "not a key\n", 0o600);

    match load(&path) {
        Err(VaultError::DecryptionFailed(msg)) => assert!(msg.contains("parse")),
        other => panic!("expected DecryptionFailed, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn insecure_permissions_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("id_ed25519");
    write_key(&path, fx::ED25519_UNENCRYPTED, 0o644);

    match load(&path) {
        Err(VaultError::InsecureIdentityPermissions(p)) => assert_eq!(p, path),
        other => panic!("expected InsecureIdentityPermissions, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn mode_0600_accepted() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("id_ed25519");
    write_key(&path, fx::ED25519_UNENCRYPTED, 0o600);
    assert!(load(&path).is_ok());
}

#[cfg(unix)]
#[test]
fn mode_0400_accepted() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("id_ed25519");
    write_key(&path, fx::ED25519_UNENCRYPTED, 0o400);
    assert!(load(&path).is_ok());
}
