use std::sync::Arc;

use loopal_protocol::{WorkflowAttemptId, WorkflowRunId};

use super::journal_support::TestJournal;
use super::scheduler_reconnect_support::recovered;
use super::scheduler_support::{coordinator as scheduler_coordinator, test_spawner};
use super::support::{coordinator, owner, request};
use crate::types::AgentExecutionRef;
use crate::workflow::recovery::WorkflowAttemptReconnect;
use crate::workflow::{WorkflowCoordinatorError, WorkflowCoordinatorMode};

fn reconnect_request() -> WorkflowAttemptReconnect {
    let (_, causation, capability) = recovered(false);
    WorkflowAttemptReconnect {
        causation,
        capability,
        execution: AgentExecutionRef::local("worker", 7),
    }
}

#[tokio::test]
async fn worker_handshake_requires_execution_mode_recovery_and_a_healthy_owner() {
    let reconnect = reconnect_request();
    let (preview, preview_task, _, _) = coordinator(WorkflowCoordinatorMode::Preview, [], []);
    assert_eq!(
        preview
            .worker_handshake(
                owner("session-handshake-preview", "root"),
                reconnect.clone(),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidOwner)
    );
    preview.shutdown().await.unwrap();
    preview_task.await.unwrap();

    let run_id = WorkflowRunId::new("wrun_handshake_poison");
    let journal = Arc::new(TestJournal::new());
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    let (spawner, _control) = test_spawner();
    let (handle, task, _, _) = scheduler_coordinator(
        [300, 301],
        [run_id],
        [WorkflowAttemptId::new("watt_handshake_poison")],
        journal,
        spawner,
    );
    assert_eq!(
        handle
            .worker_handshake(owner("bad/session", "root"), reconnect.clone())
            .await,
        Err(WorkflowCoordinatorError::InvalidOwner)
    );
    let valid_owner = owner("session-handshake-poison", "root");
    assert_eq!(
        handle
            .worker_handshake(valid_owner.clone(), reconnect.clone())
            .await,
        Err(WorkflowCoordinatorError::RecoveryRequired)
    );
    assert_eq!(
        handle
            .start(valid_owner.clone(), request("wreq_handshake_poison"))
            .await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        handle.worker_handshake(valid_owner, reconnect).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}
