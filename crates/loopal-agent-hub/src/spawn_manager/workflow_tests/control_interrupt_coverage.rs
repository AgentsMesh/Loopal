use std::sync::Arc;

use loopal_protocol::QualifiedAddress;

use super::support::{Harness, harness, harness_with_audit};
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{WorkflowSpawner, WorkflowStopStatus};

async fn replace_registry_entry_with_shadow(fixture: &Harness) -> AgentExecutionRef {
    let execution = {
        let mut hub = fixture.spawner.hub.lock().await;
        assert!(hub.registry.unregister_exact(&fixture.execution));
        hub.registry
            .register_shadow_with_parent_policy_execution(
                &fixture.execution.address.agent,
                QualifiedAddress::local("root"),
                true,
            )
            .unwrap()
    };
    let mut owners = fixture.spawner.attempts.lock().await;
    let attempt_id = owners.by_execution.remove(&fixture.execution).unwrap();
    owners
        .by_execution
        .insert(execution.clone(), attempt_id.clone());
    owners.by_attempt.get_mut(&attempt_id).unwrap().execution = execution.clone();
    execution
}

#[tokio::test]
async fn shadow_interrupt_retains_existing_cleanup_custody() {
    let fixture = harness().await;
    let execution = replace_registry_entry_with_shadow(&fixture).await;

    assert_eq!(
        fixture.spawner.interrupt(&execution).await,
        WorkflowStopStatus::Requested
    );
    assert!(
        fixture
            .spawner
            .attempts
            .lock()
            .await
            .by_execution
            .contains_key(&execution)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_registry_interrupt_observes_custody_lost_during_audit() {
    let (sink, gate) = Sink::gated();
    let fixture = harness_with_audit(Some(Arc::new(sink))).await;
    let interruption = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move { spawner.interrupt(&execution).await }
    });

    gate.wait_started().await;
    assert!(
        fixture
            .spawner
            .hub
            .lock()
            .await
            .registry
            .unregister_exact(&fixture.execution)
    );
    {
        let mut owners = fixture.spawner.attempts.lock().await;
        assert!(super::super::remove_exact_owner(
            &mut owners,
            &fixture.execution,
        ));
    }
    gate.release();

    assert_eq!(interruption.await.unwrap(), WorkflowStopStatus::Stopped);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_shadow_interrupt_observes_custody_lost_during_audit() {
    let (sink, gate) = Sink::gated();
    let fixture = harness_with_audit(Some(Arc::new(sink))).await;
    let execution = replace_registry_entry_with_shadow(&fixture).await;
    let interruption = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = execution.clone();
        async move { spawner.interrupt(&execution).await }
    });

    gate.wait_started().await;
    {
        let mut owners = fixture.spawner.attempts.lock().await;
        assert!(super::super::remove_exact_owner(&mut owners, &execution));
    }
    gate.release();

    assert_eq!(interruption.await.unwrap(), WorkflowStopStatus::Stopped);
}
