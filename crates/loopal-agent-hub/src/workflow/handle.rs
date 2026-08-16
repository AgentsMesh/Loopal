use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowRunsSnapshot, WorkflowStartRequest, WorkflowStartResponse, WorkflowWaitRequest,
    WorkflowWaitResponse,
};
use tokio::sync::{mpsc, oneshot};

use super::command::WorkflowCommand;
use super::{WorkflowCoordinatorError, WorkflowOwner};

#[path = "handle_recovery.rs"]
mod recovery;
#[path = "handle_lookup.rs"]
mod start_lookup;

#[derive(Clone, Debug)]
pub struct WorkflowCoordinatorHandle {
    pub(super) commands: mpsc::Sender<WorkflowCommand>,
}

impl WorkflowCoordinatorHandle {
    pub(crate) fn same_channel(&self, other: &Self) -> bool {
        self.commands.same_channel(&other.commands)
    }

    /// Return the authoritative active and recent workflow summaries owned by
    /// this session/root pair. Recovery is performed before the query when
    /// necessary, so cold-start callers receive a complete projection seed.
    pub async fn snapshot(
        &self,
        owner: WorkflowOwner,
    ) -> Result<WorkflowRunsSnapshot, WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Snapshot { owner, response })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    pub async fn start(
        &self,
        owner: WorkflowOwner,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowCoordinatorError> {
        super::worker_profile::validate_spec_profiles(&request.spec)?;
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Start {
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

    #[cfg(test)]
    pub(crate) async fn schedule(
        &self,
        owner: WorkflowOwner,
        run_id: loopal_protocol::WorkflowRunId,
    ) -> Result<(), WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Schedule {
                owner,
                run_id,
                response,
            })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    pub async fn cancel(
        &self,
        owner: WorkflowOwner,
        request: WorkflowCancelRequest,
    ) -> Result<WorkflowCancelResponse, WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Cancel {
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

    pub async fn tick(&self, now_unix_ms: u64) -> Result<(), WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Tick {
                now_unix_ms,
                response,
            })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    pub(crate) async fn activate_terminal_deliveries(
        &self,
        owner: WorkflowOwner,
    ) -> Result<(), WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::ActivateTerminalDeliveries { owner, response })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    pub async fn get(
        &self,
        owner: WorkflowOwner,
        request: WorkflowGetRequest,
    ) -> Result<WorkflowGetResponse, WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Get {
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

    pub async fn wait(
        &self,
        owner: WorkflowOwner,
        request: WorkflowWaitRequest,
    ) -> Result<WorkflowWaitResponse, WorkflowCoordinatorError> {
        super::wait::validate_request(&request)?;
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Subscribe {
                owner,
                run_id: request.run_id.clone(),
                response,
            })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        let revision = receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)??;
        super::wait::wait(revision, request).await
    }

    pub async fn shutdown(&self) -> Result<(), WorkflowCoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.commands
            .send(WorkflowCommand::Shutdown { response })
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| WorkflowCoordinatorError::Unavailable)?
    }

    #[cfg(test)]
    pub(crate) fn spawn_test_blocked() -> (Self, tokio::task::JoinHandle<()>, oneshot::Receiver<()>)
    {
        let (commands, mut receiver) = mpsc::channel(1);
        let (seen, notified) = oneshot::channel();
        let task = tokio::spawn(async move {
            if receiver.recv().await.is_some() {
                let _ = seen.send(());
                std::future::pending::<()>().await;
            }
        });
        (Self { commands }, task, notified)
    }
}
