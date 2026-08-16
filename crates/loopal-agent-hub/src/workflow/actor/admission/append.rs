use tokio::task::JoinHandle;

use crate::workflow::WorkflowCoordinatorError;

pub(in crate::workflow) async fn await_append(
    append: JoinHandle<Result<(), WorkflowCoordinatorError>>,
) -> Result<(), WorkflowCoordinatorError> {
    append
        .await
        .unwrap_or(Err(WorkflowCoordinatorError::JournalUnavailable))
}
