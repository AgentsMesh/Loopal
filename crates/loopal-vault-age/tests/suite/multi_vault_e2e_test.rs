//! End-to-end multi-vault tests.
//!
//! Builds real on-disk `.loopal/vaults/<name>.vault/store.age` files, drives
//! them through the real `AgeVault` + `MergedVault` pipeline, and verifies
//! the contract the runtime depends on. No mocks.

use std::sync::Arc;

use loopal_secret_runtime::MergedVault;
use loopal_vault_age::{AgeVault, list_initialized_vaults};
use loopal_vault_api::{SecretString, Vault};
use secrecy::ExposeSecret;

use crate::vault_fixtures::{PUBKEY_ALICE, Skel};

#[tokio::test]
async fn list_initialized_vaults_finds_all_initialized_real_dirs() {
    let s = Skel::new();
    s.init_vault("default", PUBKEY_ALICE).await;
    s.init_vault("production", PUBKEY_ALICE).await;
    s.init_vault("staging", PUBKEY_ALICE).await;

    let half = s.vaults_dir.join("broken.vault");
    std::fs::create_dir_all(&half).unwrap();
    std::fs::write(half.join("recipients"), "").unwrap();
    std::fs::create_dir_all(s.vaults_dir.join("notavault")).unwrap();

    let names = list_initialized_vaults(&s.vaults_dir);
    assert_eq!(names, vec!["default", "production", "staging"]);
}

#[tokio::test]
async fn e2e_merged_vault_round_trips_across_real_vaults() {
    let s = Skel::new();
    let default = Arc::new(s.init_vault("default", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    let prod = Arc::new(s.init_vault("production", PUBKEY_ALICE).await) as Arc<dyn Vault>;

    default
        .put("openai_key", SecretString::from("sk-default"))
        .await
        .unwrap();
    prod.put("db_password", SecretString::from("prod-db-pwd-1"))
        .await
        .unwrap();

    let merged = MergedVault::new(
        ("default".to_string(), default),
        vec![("production".to_string(), prod)],
    );

    assert_eq!(merged.list_names().await, vec!["db_password", "openai_key"]);
    assert_eq!(
        merged.get("openai_key").await.unwrap().expose_secret(),
        "sk-default"
    );
    assert_eq!(
        merged.get("db_password").await.unwrap().expose_secret(),
        "prod-db-pwd-1"
    );
}

#[tokio::test]
async fn e2e_conflict_default_wins_with_real_ciphertext() {
    let s = Skel::new();
    let default = Arc::new(s.init_vault("default", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    let prod = Arc::new(s.init_vault("production", PUBKEY_ALICE).await) as Arc<dyn Vault>;

    default
        .put("shared", SecretString::from("from-default"))
        .await
        .unwrap();
    prod.put("shared", SecretString::from("from-production"))
        .await
        .unwrap();

    let merged = MergedVault::new(
        ("default".to_string(), default),
        vec![("production".to_string(), prod)],
    );

    assert_eq!(
        merged.get("shared").await.unwrap().expose_secret(),
        "from-default"
    );
}

#[tokio::test]
async fn e2e_put_invalidates_cache_and_persists_to_disk() {
    let s = Skel::new();
    let default = Arc::new(s.init_vault("default", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    let prod = Arc::new(s.init_vault("production", PUBKEY_ALICE).await) as Arc<dyn Vault>;

    let merged = MergedVault::new(
        ("default".to_string(), default.clone()),
        vec![("production".to_string(), prod)],
    );

    assert!(merged.list_names().await.is_empty());

    merged
        .put("new_key", SecretString::from("plaintext-v1"))
        .await
        .unwrap();
    assert_eq!(merged.list_names().await, vec!["new_key"]);

    let fresh = AgeVault::new(
        s.vaults_dir.join("default.vault").join("store.age"),
        s.vaults_dir.join("default.vault").join("recipients"),
        s.identity.clone(),
    );
    assert_eq!(
        fresh.get("new_key").await.unwrap().expose_secret(),
        "plaintext-v1"
    );
}

#[tokio::test]
async fn e2e_delete_routes_to_owning_vault() {
    let s = Skel::new();
    let default = Arc::new(s.init_vault("default", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    let prod = Arc::new(s.init_vault("production", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    prod.put("only_in_prod", SecretString::from("v_for_prod"))
        .await
        .unwrap();

    let merged = MergedVault::new(
        ("default".to_string(), default),
        vec![("production".to_string(), prod.clone())],
    );

    assert_eq!(merged.list_names().await, vec!["only_in_prod"]);
    merged.delete("only_in_prod").await.unwrap();

    let fresh_prod = AgeVault::new(
        s.vaults_dir.join("production.vault").join("store.age"),
        s.vaults_dir.join("production.vault").join("recipients"),
        s.identity.clone(),
    );
    assert!(fresh_prod.get("only_in_prod").await.is_none());
}

#[tokio::test]
async fn e2e_rekey_rewrites_all_vault_ciphertext() {
    let s = Skel::new();
    let d = Arc::new(s.init_vault("default", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    let p = Arc::new(s.init_vault("production", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    d.put("k_default", SecretString::from("vvvvvvvv"))
        .await
        .unwrap();
    p.put("k_prod", SecretString::from("wwwwwwww"))
        .await
        .unwrap();

    let default_store = s.vaults_dir.join("default.vault").join("store.age");
    let prod_store = s.vaults_dir.join("production.vault").join("store.age");
    let before_d = std::fs::read(&default_store).unwrap();
    let before_p = std::fs::read(&prod_store).unwrap();

    let merged = MergedVault::new(
        ("default".to_string(), d),
        vec![("production".to_string(), p)],
    );
    merged.rekey().await.unwrap();

    let after_d = std::fs::read(&default_store).unwrap();
    let after_p = std::fs::read(&prod_store).unwrap();
    assert_ne!(before_d, after_d);
    assert_ne!(before_p, after_p);

    let fresh_d = AgeVault::new(
        default_store,
        s.vaults_dir.join("default.vault").join("recipients"),
        s.identity.clone(),
    );
    let fresh_p = AgeVault::new(
        prod_store,
        s.vaults_dir.join("production.vault").join("recipients"),
        s.identity.clone(),
    );
    assert_eq!(
        fresh_d.get("k_default").await.unwrap().expose_secret(),
        "vvvvvvvv"
    );
    assert_eq!(
        fresh_p.get("k_prod").await.unwrap().expose_secret(),
        "wwwwwwww"
    );
}

#[tokio::test]
async fn e2e_single_vault_no_merged_layer_works() {
    let s = Skel::new();
    let v = s.init_vault("default", PUBKEY_ALICE).await;
    v.put("only", SecretString::from("12345678")).await.unwrap();
    assert_eq!(v.get("only").await.unwrap().expose_secret(), "12345678");
    assert_eq!(v.list_names().await, vec!["only"]);
}
