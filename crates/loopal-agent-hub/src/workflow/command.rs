use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowRunId, WorkflowRunSnapshot, WorkflowRunsSnapshot, WorkflowStartLookupRequest,
    WorkflowStartLookupResponse, WorkflowStartRequest, WorkflowStartResponse,
};
use tokio::sync::{oneshot, watch};

use super::recovery::{WorkflowAttemptReconnect, WorkflowAttemptReconnectResponse};
use super::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) enum WorkflowCommand {
    Recover {
        owner: WorkflowOwner,
        response: oneshot::Sender<Result<usize, WorkflowCoordinatorError>>,
    },
    Resume {
        owner: WorkflowOwner,
        response: oneshot::Sender<Result<(), WorkflowCoordinatorError>>,
    },
    Reconnect {
        owner: WorkflowOwner,
        request: WorkflowAttemptReconnect,
        response:
            oneshot::Sender<Result<WorkflowAttemptReconnectResponse, WorkflowCoordinatorError>>,
    },
    WorkerHandshake {
        owner: WorkflowOwner,
        request: WorkflowAttemptReconnect,
        response: oneshot::Sender<
            Result<loopal_protocol::WorkflowWorkerHandshakeResponse, WorkflowCoordinatorError>,
        >,
    },
    Snapshot {
        owner: WorkflowOwner,
        response: oneshot::Sender<Result<WorkflowRunsSnapshot, WorkflowCoordinatorError>>,
    },
    Start {
        owner: WorkflowOwner,
        request: WorkflowStartRequest,
        response: oneshot::Sender<Result<WorkflowStartResponse, WorkflowCoordinatorError>>,
    },
    LookupStart {
        owner: WorkflowOwner,
        request: WorkflowStartLookupRequest,
        response: oneshot::Sender<Result<WorkflowStartLookupResponse, WorkflowCoordinatorError>>,
    },
    Get {
        owner: WorkflowOwner,
        request: WorkflowGetRequest,
        response: oneshot::Sender<Result<WorkflowGetResponse, WorkflowCoordinatorError>>,
    },
    #[cfg(test)]
    Schedule {
        owner: WorkflowOwner,
        run_id: WorkflowRunId,
        response: oneshot::Sender<Result<(), WorkflowCoordinatorError>>,
    },
    #[cfg(test)]
    Pause {
        started: oneshot::Sender<()>,
        release: oneshot::Receiver<()>,
    },
    WorkerPrepared {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        prepared: super::scheduler::WorkflowPreparedDelivery,
    },
    WorkerPreparationTimedOut {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        failure: super::scheduler::WorkflowSpawnFailure,
    },
    WorkerPreparationAborted {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        status: super::scheduler::WorkflowCleanupStatus,
    },
    FinalizePreparationAbort {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
    },
    PreparationDeliveryFinished {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
    },
    LatePreparationShutdown {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        status: super::scheduler::WorkflowCleanupStatus,
    },
    WorkerActivated {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        result: Result<(), super::scheduler::WorkflowActivationFailure>,
    },
    WorkerFinished {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        outcome: super::scheduler::WorkflowWorkerOutcome,
    },
    WorkerOutcomeLost {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
    },
    WorkerStopped {
        owner: WorkflowOwner,
        key: super::scheduler::AttemptKey,
        execution: crate::types::AgentExecutionRef,
        status: super::scheduler::WorkflowCleanupStatus,
    },
    Cancel {
        owner: WorkflowOwner,
        request: WorkflowCancelRequest,
        response: oneshot::Sender<Result<WorkflowCancelResponse, WorkflowCoordinatorError>>,
    },
    Subscribe {
        owner: WorkflowOwner,
        run_id: WorkflowRunId,
        response: oneshot::Sender<
            Result<Option<watch::Receiver<WorkflowRunSnapshot>>, WorkflowCoordinatorError>,
        >,
    },
    Tick {
        now_unix_ms: u64,
        response: oneshot::Sender<Result<(), WorkflowCoordinatorError>>,
    },
    ActivateTerminalDeliveries {
        owner: WorkflowOwner,
        response: oneshot::Sender<Result<(), WorkflowCoordinatorError>>,
    },
    TerminalDeliveryResolved {
        owner: WorkflowOwner,
        delivery_id: loopal_protocol::WorkflowTerminalDeliveryId,
        result: Result<loopal_protocol::WorkflowTerminalDisposition, String>,
        task_panicked: bool,
    },
    Shutdown {
        response: oneshot::Sender<Result<(), WorkflowCoordinatorError>>,
    },
}
