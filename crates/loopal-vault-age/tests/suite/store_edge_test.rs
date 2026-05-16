//! `AgeVault` edge cases: invalid names, missing recipients, encrypted
//! identity rejection. These don't need the standard happy-path harness.

use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

use loopal_vault_age::{AgeVault, Recipients};
use loopal_vault_api::Vault;
use secrecy::SecretString;

use crate::ssh_fixtures as fx;
use crate::store_fixtures::{PUBKEY_ALICE, build_harness, write_key};

#[tokio::test]
async fn put_invalid_name_rejected() {
    let h = build_harness();
    let err = h
        .store
        .put("UpperCase", SecretString::from("v"))
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("UpperCase"), "got: {msg}");
    assert!(msg.contains("invalid secret name"), "got: {msg}");
}

#[tokio::test]
async fn empty_recipients_file_yields_encryption_error() {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    write_key(&key_path, fx::ED25519_UNENCRYPTED, 0o600);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());

    let recipients_path = dir.path().join(".age-recipients");
    fs::write(&recipients_path, "# empty\n").unwrap();

    let vault_path = dir.path().join("secrets.yaml.age");
    let store = AgeVault::new(vault_path, recipients_path, identity);

    let err = store
        .put("x", SecretString::from("12345678"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no recipients"), "got: {err}");
}

#[tokio::test]
async fn encrypted_identity_without_agent_rejected() {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    write_key(&key_path, fx::ED25519_AES256_CBC, 0o600);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());

    let recipients_path = dir.path().join(".age-recipients");
    let mut rec = Recipients::new();
    rec.add_line(PUBKEY_ALICE).unwrap();
    rec.write(&recipients_path).unwrap();

    let vault_path = dir.path().join("secrets.yaml.age");
    let store = AgeVault::new(vault_path, recipients_path, identity);

    // ensure_usable rejects on first cache load, list_names returns empty.
    assert!(store.list_names().await.is_empty());

    let err = store
        .put("x", SecretString::from("12345678"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("passphrase-protected"),
        "got: {err}"
    );
}
