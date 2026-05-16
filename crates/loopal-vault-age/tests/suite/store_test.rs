//! Core `AgeVault` round-trip tests — put/get/delete/list/rekey happy paths
//! and well-formed-input cases. Error paths and security edge cases live in
//! `store_edge_test.rs`.

use std::fs;

use loopal_vault_age::AgeVault;
use loopal_vault_api::Vault;
use secrecy::{ExposeSecret, SecretString};

use crate::store_fixtures::build_harness;

#[tokio::test]
async fn get_on_empty_store_returns_none() {
    let h = build_harness();
    assert!(h.store.get("anything").await.is_none());
}

#[tokio::test]
async fn put_then_get_round_trips() {
    let h = build_harness();
    h.store
        .put("openai_key", SecretString::from("sk-abc12345"))
        .await
        .unwrap();
    let got = h.store.get("openai_key").await.unwrap();
    assert_eq!(got.expose_secret(), "sk-abc12345");
}

#[tokio::test]
async fn put_persists_across_store_instances() {
    let h = build_harness();
    h.store
        .put("hf_token", SecretString::from("hf-tokenvalue"))
        .await
        .unwrap();
    assert!(h.vault.exists());

    let store2 = AgeVault::new(h.vault.clone(), h.recipients.clone(), h.identity.clone());
    let got = store2.get("hf_token").await.unwrap();
    assert_eq!(got.expose_secret(), "hf-tokenvalue");
}

#[tokio::test]
async fn delete_removes_secret() {
    let h = build_harness();
    h.store
        .put("temp", SecretString::from("123abc456"))
        .await
        .unwrap();
    h.store.delete("temp").await.unwrap();
    assert!(h.store.get("temp").await.is_none());
}

#[tokio::test]
async fn delete_missing_key_is_no_op() {
    let h = build_harness();
    h.store.delete("ghost").await.unwrap();
}

#[tokio::test]
async fn list_names_returns_all_keys() {
    let h = build_harness();
    h.store
        .put("first", SecretString::from("11111111"))
        .await
        .unwrap();
    h.store
        .put("second", SecretString::from("22222222"))
        .await
        .unwrap();
    let mut names = h.store.list_names().await;
    names.sort();
    assert_eq!(names, vec!["first".to_string(), "second".to_string()]);
}

#[tokio::test]
async fn rekey_rewrites_ciphertext() {
    let h = build_harness();
    h.store
        .put("k", SecretString::from("v_v_v_v_v"))
        .await
        .unwrap();
    let before = fs::read(&h.vault).unwrap();

    h.store.rekey().await.unwrap();
    let after = fs::read(&h.vault).unwrap();

    assert_ne!(before, after, "rekey should produce a fresh nonce");
    let got = h.store.get("k").await.unwrap();
    assert_eq!(got.expose_secret(), "v_v_v_v_v");
}

#[cfg(unix)]
#[tokio::test]
async fn vault_file_mode_is_0600() {
    use std::os::unix::fs::PermissionsExt;
    let h = build_harness();
    h.store
        .put("k", SecretString::from("12345678"))
        .await
        .unwrap();
    let mode = fs::metadata(&h.vault).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
