use loopal_protocol::{WorkflowStartLookupRequest, WorkflowStartLookupResponse};
use tokio::sync::oneshot;

use super::super::command::WorkflowCommand;
use super::super::{WorkflowCoordinatorError, WorkflowOwner};
use super::WorkflowCoordinatorHandle;

impl WorkflowCoordinatorHandle {
    pub async fn lookup_start(
        &self,
        owner: WorkflowOwner,
        request: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::LookupStart {
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
