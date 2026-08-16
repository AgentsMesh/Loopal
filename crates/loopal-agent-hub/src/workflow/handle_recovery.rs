use tokio::sync::oneshot;

use super::super::command::WorkflowCommand;
use super::super::recovery::{WorkflowAttemptReconnect, WorkflowAttemptReconnectResponse};
use super::super::{WorkflowCoordinatorError, WorkflowOwner};
use super::WorkflowCoordinatorHandle;

impl WorkflowCoordinatorHandle {
    pub async fn recover(&self, owner: WorkflowOwner) -> Result<usize, WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Recover { owner, response })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    pub(crate) async fn resume(
        &self,
        owner: WorkflowOwner,
    ) -> Result<(), WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Resume { owner, response })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    #[allow(dead_code)]
    pub(crate) async fn reconnect_attempt(
        &self,
        owner: WorkflowOwner,
        request: WorkflowAttemptReconnect,
    ) -> Result<WorkflowAttemptReconnectResponse, WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Reconnect {
                owner,
                request,
                response,
            })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    pub(crate) async fn worker_handshake(
        &self,
        owner: WorkflowOwner,
        request: WorkflowAttemptReconnect,
    ) -> Result<loopal_protocol::WorkflowWorkerHandshakeResponse, WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::WorkerHandshake {
                owner,
                request,
                response,
            })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }
}
