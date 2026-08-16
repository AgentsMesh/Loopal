use std::sync::Arc;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptCapability, WorkflowAttemptState, WorkflowEventPayload,
    WorkflowOutput, WorkflowRunState, WorkflowWorkerHandshakeDisposition,
};

use super::journal_support::TestJournal;
use super::scheduler_reconnect_support::{begin_handshake, coordinator, recovered};
use super::scheduler_support::{SpawnerEffect, prepared_worker};
use super::support::{TestClock, owner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowCleanupStatus, WorkflowRecoveryAdoptionError, WorkflowWorkerOutcome,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

#[tokio::test]
async fn missing_custody_rejects_adoption_and_preserves_recovery_deadline() {
    let (run, causation, capability) = recovered(true);
    let journal = Arc::new(TestJournal::new());
    let (handle, task, control) = coordinator(
        journal.clone(),
        Arc::new(TestClock::new([800, 801, 802, 903])),
        run,
    );
    let owner = owner("session-missing-custody", "root");
    handle.recover(owner.clone()).await.unwrap();

    let wrong = begin_handshake(
        handle.clone(),
        owner.clone(),
        causation.clone(),
        WorkflowAttemptCapability::parse("22".repeat(32)).unwrap(),
        AgentExecutionRef::local("worker", 7),
    )
    .await;
    assert_eq!(wrong, Err(WorkflowCoordinatorError::InvalidExecutionLease));
    control.assert_idle().await;

    let handshake = tokio::spawn(begin_handshake(
        handle.clone(),
        owner.clone(),
        causation,
        capability,
        AgentExecutionRef::local("worker", 7),
    ));
    let SpawnerEffect::AdoptRecovered { response, .. } = control.next().await else {
        panic!("expected recovery custody acquisition")
    };
    assert!(
        response
            .send(Err(WorkflowRecoveryAdoptionError::MissingCustody))
            .is_ok()
    );
    assert_eq!(
        handshake.await.unwrap(),
        Err(WorkflowCoordinatorError::StaleExecutionLease)
    );

    handle.tick(900).await.unwrap();
    assert!(matches!(
        last_payload(&journal),
        WorkflowEventPayload::AttemptFailed { .. }
    ));
    assert_eq!(
        summary_state(&handle, owner).await,
        WorkflowRunState::Failed
    );
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn adopted_outcome_is_owned_and_terminalizes_the_recovered_attempt() {
    let (run, causation, capability) = recovered(true);
    let journal = Arc::new(TestJournal::new());
    let (handle, task, control) = coordinator(
        journal.clone(),
        Arc::new(TestClock::new([800, 801, 802])),
        run,
    );
    let owner = owner("session-adopt-outcome", "root");
    handle.recover(owner.clone()).await.unwrap();
    let execution = AgentExecutionRef::local("worker", 7);
    let handshake = tokio::spawn(begin_handshake(
        handle.clone(),
        owner.clone(),
        causation.clone(),
        capability,
        execution.clone(),
    ));
    let SpawnerEffect::AdoptRecovered { request, response } = control.next().await else {
        panic!("expected recovery custody acquisition")
    };
    assert_eq!(request.owner, owner);
    assert_eq!(request.causation, causation);
    assert_eq!(request.execution, execution);
    assert!(request.output_contract.is_some());
    let (worker, outcome) = prepared_worker("worker", 7);
    assert!(response.send(Ok(worker)).is_ok());
    let adopted = handshake.await.unwrap().unwrap();
    assert_eq!(
        adopted.disposition,
        WorkflowWorkerHandshakeDisposition::Recovered
    );
    assert_eq!(adopted.attempt_state, WorkflowAttemptState::Running);

    outcome
        .send(WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(Some("done".into())),
            output: Some(WorkflowOutput::Text("done".into())),
        })
        .unwrap();
    journal.wait_for_event_batches(1).await;
    assert!(matches!(
        last_payload(&journal),
        WorkflowEventPayload::AttemptSucceeded { .. }
    ));
    assert_eq!(
        summary_state(&handle, owner).await,
        WorkflowRunState::Succeeded
    );
    control.assert_idle().await;
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn journal_failure_after_custody_claim_contains_exact_execution() {
    let (run, causation, capability) = recovered(false);
    let journal = Arc::new(TestJournal::new());
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    let (handle, task, control) = coordinator(
        journal.clone(),
        Arc::new(TestClock::new([800, 801, 802])),
        run,
    );
    let owner = owner("session-adopt-journal-failure", "root");
    handle.recover(owner.clone()).await.unwrap();
    let handshake = tokio::spawn(begin_handshake(
        handle.clone(),
        owner,
        causation,
        capability,
        AgentExecutionRef::local("worker", 7),
    ));
    let SpawnerEffect::AdoptRecovered { response, .. } = control.next().await else {
        panic!("expected recovery custody acquisition")
    };
    let (worker, outcome) = prepared_worker("worker", 7);
    assert!(response.send(Ok(worker)).is_ok());
    assert_eq!(
        handshake.await.unwrap(),
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    let SpawnerEffect::Shutdown {
        execution,
        response,
    } = control.next().await
    else {
        panic!("expected exact containment after journal failure")
    };
    assert_eq!(execution, AgentExecutionRef::local("worker", 7));
    response.send(WorkflowCleanupStatus::Confirmed).unwrap();
    assert!(outcome.is_closed());
    assert!(journal.events().is_empty());
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}

async fn summary_state(
    handle: &crate::workflow::WorkflowCoordinatorHandle,
    owner: WorkflowOwner,
) -> WorkflowRunState {
    handle.snapshot(owner).await.unwrap().recent[0].state
}

fn last_payload(journal: &TestJournal) -> WorkflowEventPayload {
    journal
        .events()
        .into_iter()
        .last()
        .unwrap()
        .2
        .last()
        .unwrap()
        .payload
        .clone()
}
