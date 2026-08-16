use std::sync::Arc;
use std::sync::atomic::Ordering;

use loopal_protocol::AgentCompletion;

use super::fake_worker::{FakeProcess, Probe, spawn_peer};
use super::requests::{causation, request};
use super::support::spawner_with_root;
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::workflow::scheduler::{WorkflowSpawner, WorkflowWorkerOutcome};

#[tokio::test]
async fn prepared_fake_process_uses_the_complete_publication_and_monitor_path() {
    let spawner = spawner_with_root(Some(Arc::new(loopal_vault_api::NoopAuditSink))).await;
    let probe = Arc::new(Probe::default());
    probe.reply_to_shutdown.store(true, Ordering::SeqCst);
    let (worker_transport, hub_transport) = loopal_ipc::duplex_pair();
    spawn_peer(worker_transport, probe.clone());
    let causation = causation(
        "wrun_prepare_full",
        "wnode_prepare_full",
        "watt_prepare_full",
    );

    let worker = super::super::prepare::run_with_process_for_test(
        &spawner,
        request(causation.clone()),
        FakeProcess::new(hub_transport, probe.clone()),
    )
    .await
    .unwrap();
    let execution = worker.execution.clone();
    {
        let owners = spawner.attempts.lock().await;
        assert_eq!(
            owners.by_execution.get(&execution),
            Some(&causation.attempt_id)
        );
        assert_eq!(
            owners.by_attempt[&causation.attempt_id].execution,
            execution
        );
        assert!(
            owners.by_attempt[&causation.attempt_id].phase == super::super::AttemptPhase::Prepared
        );
    }
    let facts = spawner
        .hub
        .lock()
        .await
        .registry
        .runtime_facts(&execution)
        .unwrap()
        .clone();
    assert_eq!(facts.workflow_permission_causation, Some(causation.clone()));
    assert!(facts.workflow_attempt_capability_digest.is_some());

    assert!(spawner.activate(&execution).await.is_ok());
    assert_eq!(probe.starts.load(Ordering::SeqCst), 1);
    let connection = spawner.attempts.lock().await.by_attempt[&causation.attempt_id]
        .control
        .connection
        .clone();
    crate::finish::finish_and_deliver_exact(
        &spawner.hub,
        &execution.address.agent,
        AgentCompletion::goal(Some("prepared result".into())),
        &connection,
        &execution,
    )
    .await;

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(1), worker.outcome)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(outcome, WorkflowWorkerOutcome::Succeeded { .. }));
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let owners = spawner.attempts.lock().await;
            if owners.by_attempt.is_empty() && owners.by_execution.is_empty() {
                return;
            }
            drop(owners);
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execution_index_collision_terminates_the_unpublished_process() {
    let (sink, gate) = Sink::gated();
    let spawner = spawner_with_root(Some(Arc::new(sink))).await;
    let probe = Arc::new(Probe::default());
    let (worker_transport, hub_transport) = loopal_ipc::duplex_pair();
    spawn_peer(worker_transport, probe.clone());
    let causation = causation(
        "wrun_prepare_collision",
        "wnode_prepare_collision",
        "watt_prepare_collision",
    );
    let process_probe = probe.clone();
    let preparation = tokio::spawn({
        let spawner = spawner.clone();
        let causation = causation.clone();
        async move {
            super::super::prepare::run_with_process_for_test(
                &spawner,
                request(causation),
                FakeProcess::new(hub_transport, process_probe),
            )
            .await
        }
    });
    gate.wait_started().await;
    let mut owners = spawner.attempts.lock().await;
    gate.release();
    let worker_name = format!("workflow-{}", causation.attempt_id);
    let execution = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if let Some(execution) = spawner
                .hub
                .lock()
                .await
                .registry
                .current_execution(&worker_name)
            {
                return execution;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let collision = loopal_protocol::WorkflowAttemptId::new("watt_existing_collision");
    owners
        .by_execution
        .insert(execution.clone(), collision.clone());
    drop(owners);

    let failure = match preparation.await.unwrap() {
        Err(failure) => failure,
        Ok(_) => panic!("execution index collision must cancel preparation"),
    };
    assert!(failure.failure.reason.contains("cancelled"));
    assert!(
        spawner
            .hub
            .lock()
            .await
            .registry
            .current_execution(&worker_name)
            .is_none()
    );
    assert!(probe.process_stopped.load(Ordering::SeqCst));
    let mut owners = spawner.attempts.lock().await;
    assert_eq!(owners.by_execution.remove(&execution), Some(collision));
    assert!(owners.preparing.is_empty());
    assert!(owners.by_attempt.is_empty());
}
