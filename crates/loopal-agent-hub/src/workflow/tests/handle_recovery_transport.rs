use loopal_protocol::WorkflowAttemptState;
use tokio::sync::mpsc;

use super::scheduler_reconnect_support::recovered;
use super::support::owner;
use crate::types::AgentExecutionRef;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::recovery::{WorkflowAttemptReconnect, WorkflowAttemptReconnectResponse};
use crate::workflow::{WorkflowCoordinatorError, WorkflowCoordinatorHandle};

fn reconnect_request() -> WorkflowAttemptReconnect {
    let (_, causation, capability) = recovered(false);
    WorkflowAttemptReconnect {
        causation,
        capability,
        execution: AgentExecutionRef::local("worker", 7),
    }
}

#[tokio::test]
async fn recovery_handle_fails_closed_for_command_and_response_channel_loss() {
    let owner = owner("session-handle-transport", "root");
    let reconnect = reconnect_request();
    let (commands, receiver) = mpsc::channel(1);
    drop(receiver);
    let closed = WorkflowCoordinatorHandle { commands };

    assert_eq!(
        closed.recover(owner.clone()).await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
    assert_eq!(
        closed.resume(owner.clone()).await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
    assert_eq!(
        closed
            .reconnect_attempt(owner.clone(), reconnect.clone())
            .await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
    assert_eq!(
        closed
            .worker_handshake(owner.clone(), reconnect.clone())
            .await,
        Err(WorkflowCoordinatorError::Unavailable)
    );

    let (commands, mut receiver) = mpsc::channel(1);
    let handle = WorkflowCoordinatorHandle { commands };

    let call = tokio::spawn({
        let handle = handle.clone();
        let owner = owner.clone();
        async move { handle.recover(owner).await }
    });
    let Some(WorkflowCommand::Recover { response, .. }) = receiver.recv().await else {
        panic!("expected recover command")
    };
    drop(response);
    assert_eq!(
        call.await.unwrap(),
        Err(WorkflowCoordinatorError::Unavailable)
    );

    let call = tokio::spawn({
        let handle = handle.clone();
        let owner = owner.clone();
        async move { handle.resume(owner).await }
    });
    let Some(WorkflowCommand::Resume { response, .. }) = receiver.recv().await else {
        panic!("expected resume command")
    };
    drop(response);
    assert_eq!(
        call.await.unwrap(),
        Err(WorkflowCoordinatorError::Unavailable)
    );

    let call = tokio::spawn({
        let handle = handle.clone();
        let owner = owner.clone();
        let reconnect = reconnect.clone();
        async move { handle.reconnect_attempt(owner, reconnect).await }
    });
    let Some(WorkflowCommand::Reconnect { response, .. }) = receiver.recv().await else {
        panic!("expected reconnect command")
    };
    drop(response);
    assert_eq!(
        call.await.unwrap(),
        Err(WorkflowCoordinatorError::Unavailable)
    );

    let call = tokio::spawn({
        let handle = handle.clone();
        let owner = owner.clone();
        let reconnect = reconnect.clone();
        async move { handle.worker_handshake(owner, reconnect).await }
    });
    let Some(WorkflowCommand::WorkerHandshake { response, .. }) = receiver.recv().await else {
        panic!("expected worker handshake command")
    };
    drop(response);
    assert_eq!(
        call.await.unwrap(),
        Err(WorkflowCoordinatorError::Unavailable)
    );

    let expected = WorkflowAttemptReconnectResponse {
        execution: reconnect.execution.clone(),
        attempt_state: WorkflowAttemptState::Running,
    };
    let call = tokio::spawn({
        let handle = handle.clone();
        async move { handle.reconnect_attempt(owner, reconnect).await }
    });
    let Some(WorkflowCommand::Reconnect { response, .. }) = receiver.recv().await else {
        panic!("expected reconnect command")
    };
    response.send(Ok(expected.clone())).unwrap();
    assert_eq!(call.await.unwrap().unwrap(), expected);
}
