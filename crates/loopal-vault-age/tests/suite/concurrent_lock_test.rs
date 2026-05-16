//! Concurrent vault access tests — validates the cross-process O_EXCL lock
//! serializes writes and that reads stay consistent during concurrent writes.

use std::sync::Arc;

use loopal_vault_age::AgeVault;
use loopal_vault_api::{SecretString, Vault};
use secrecy::ExposeSecret;

use crate::vault_fixtures::{PUBKEY_ALICE, Skel};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_puts_to_same_vault_serialize_via_cross_process_lock() {
    // Two writes hitting the same AgeVault at the same time must serialize
    // through the vault_io O_EXCL lockfile; neither write should be lost.
    let s = Skel::new();
    let v = Arc::new(s.init_vault("default", PUBKEY_ALICE).await) as Arc<dyn Vault>;

    let v1 = v.clone();
    let v2 = v.clone();
    let v3 = v.clone();
    let t1 = tokio::spawn(async move { v1.put("key_one", SecretString::from("value_one")).await });
    let t2 = tokio::spawn(async move { v2.put("key_two", SecretString::from("value_two")).await });
    let t3 =
        tokio::spawn(async move { v3.put("key_three", SecretString::from("value_three")).await });
    t1.await.unwrap().unwrap();
    t2.await.unwrap().unwrap();
    t3.await.unwrap().unwrap();

    let fresh = AgeVault::new(
        s.vaults_dir.join("default.vault").join("store.age"),
        s.vaults_dir.join("default.vault").join("recipients"),
        s.identity.clone(),
    );
    let mut names = fresh.list_names().await;
    names.sort();
    assert_eq!(names, vec!["key_one", "key_three", "key_two"]);
    assert_eq!(
        fresh.get("key_one").await.unwrap().expose_secret(),
        "value_one"
    );
    assert_eq!(
        fresh.get("key_two").await.unwrap().expose_secret(),
        "value_two"
    );
    assert_eq!(
        fresh.get("key_three").await.unwrap().expose_secret(),
        "value_three"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_put_and_get_does_not_corrupt_state() {
    // While one task is writing, another reading must either see the old
    // state or the new state — never a partial write.
    let s = Skel::new();
    let v = Arc::new(s.init_vault("default", PUBKEY_ALICE).await) as Arc<dyn Vault>;
    v.put("initial", SecretString::from("starter_value"))
        .await
        .unwrap();

    let writer = v.clone();
    let reader = v.clone();
    let w = tokio::spawn(async move {
        for i in 0..5 {
            writer
                .put(
                    &format!("k_{i}"),
                    SecretString::from(format!("v_{i}_long_enough")),
                )
                .await
                .unwrap();
        }
    });
    let r = tokio::spawn(async move {
        for _ in 0..5 {
            let v = reader.get("initial").await;
            assert!(v.is_some());
            assert_eq!(v.unwrap().expose_secret(), "starter_value");
        }
    });
    w.await.unwrap();
    r.await.unwrap();

    let fresh = AgeVault::new(
        s.vaults_dir.join("default.vault").join("store.age"),
        s.vaults_dir.join("default.vault").join("recipients"),
        s.identity.clone(),
    );
    assert_eq!(
        fresh.get("initial").await.unwrap().expose_secret(),
        "starter_value"
    );
    for i in 0..5 {
        let key = format!("k_{i}");
        assert!(fresh.get(&key).await.is_some(), "lost write: {key}");
    }
}
