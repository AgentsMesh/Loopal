use std::sync::Arc;

use loopal_protocol::{AgentCompletion, WorkflowOutput, WorkflowWorkerHandshakeDisposition};

use super::journal_support::TestJournal;
use super::scheduler_reconnect_support::{begin_handshake, coordinator, recovered};
use super::scheduler_support::{SpawnerEffect, prepared_worker};
use super::support::{TestClock, owner};
use crate::types::AgentExecutionRef;
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::scheduler::WorkflowWorkerOutcome;

#[tokio::test]
async fn recovered_worker_handshake_is_single_use() {
    let (run, causation, capability) = recovered(true);
    let journal = Arc::new(TestJournal::new());
    let (handle, task, control) = coordinator(
        journal.clone(),
        Arc::new(TestClock::new([400, 401, 402])),
        run,
    );
    let owner = owner("session-handshake-single-use", "root");
    handle.recover(owner.clone()).await.unwrap();
    let execution = AgentExecutionRef::local("worker", 7);
    let handshake = tokio::spawn(begin_handshake(
        handle.clone(),
        owner.clone(),
        causation.clone(),
        capability.clone(),
        execution.clone(),
    ));
    let SpawnerEffect::AdoptRecovered { response, .. } = control.next().await else {
        panic!("expected recovery custody acquisition")
    };
    let (worker, outcome) = prepared_worker("worker", 7);
    assert!(response.send(Ok(worker)).is_ok());
    let adopted = handshake.await.unwrap().unwrap();
    assert_eq!(
        adopted.disposition,
        WorkflowWorkerHandshakeDisposition::Recovered
    );

    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            causation,
            capability,
            execution,
        )
        .await,
        Err(WorkflowCoordinatorError::StaleExecutionLease)
    );
    outcome
        .send(WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(Some("done".into())),
            output: Some(WorkflowOutput::Text("done".into())),
        })
        .unwrap();
    journal.wait_for_event_batches(1).await;
    control.assert_idle().await;
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}
