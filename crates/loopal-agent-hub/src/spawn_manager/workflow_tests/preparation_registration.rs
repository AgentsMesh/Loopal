use std::sync::Arc;

use super::requests::causation;
use super::support::spawner_with_root;
use crate::spawn_manager::workflow::PreparationOwner;

#[tokio::test]
async fn dropped_armed_registration_removes_only_its_exact_preparation() {
    let spawner = spawner_with_root(Some(Arc::new(loopal_vault_api::NoopAuditSink))).await;
    let attempt = causation("wrun_drop", "wnode_drop", "watt_drop").attempt_id;
    let original = Arc::new(PreparationOwner::new(causation(
        "wrun_drop",
        "wnode_drop",
        "watt_drop",
    )));
    spawner
        .attempts
        .lock()
        .await
        .preparing
        .insert(attempt.clone(), original.clone());
    let registration = super::super::prepare::registration::PreparationRegistration::new(
        &spawner,
        attempt.clone(),
        &original,
    );
    drop(registration);

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while spawner
            .attempts
            .lock()
            .await
            .preparing
            .contains_key(&attempt)
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    let replacement = Arc::new(PreparationOwner::new(causation(
        "wrun_replacement",
        "wnode_replacement",
        "watt_drop",
    )));
    spawner
        .attempts
        .lock()
        .await
        .preparing
        .insert(attempt.clone(), original.clone());
    let stale = super::super::prepare::registration::PreparationRegistration::new(
        &spawner,
        attempt.clone(),
        &original,
    );
    spawner
        .attempts
        .lock()
        .await
        .preparing
        .insert(attempt.clone(), replacement.clone());
    drop(stale);
    tokio::task::yield_now().await;
    assert!(Arc::ptr_eq(
        &spawner.attempts.lock().await.preparing[&attempt],
        &replacement
    ));
}
