use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use loopal_protocol::{
    AgentCompletion, QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode,
    WorkflowAttemptCapability, WorkflowAttemptFailure, WorkflowAttemptId, WorkflowEvent,
    WorkflowEventPayload, WorkflowFailureClass, WorkflowLimits, WorkflowNodeId, WorkflowNodeState,
    WorkflowOutputContract, WorkflowPermissionCausation, WorkflowRequestRecord, WorkflowRunId,
    WorkflowRunSnapshot, WorkflowRunState, WorkflowSpec, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowWorkerProfileRef,
};
use tokio::sync::{mpsc, oneshot};

use super::super::{
    WorkflowCoordinator, WorkflowCoordinatorMode, WorkflowRuntimeConfig, WorkflowTrustedCeilings,
};
use super::{callbacks, dispatch, drainage, recovery, stop};
use crate::types::AgentExecutionRef;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::journal::{
    StartJournalRecord, WorkflowJournalDeliveryAckOutcome, WorkflowJournalDeliveryIntentOutcome,
    WorkflowJournalStorage,
};
use crate::workflow::recovery::{RecoveredOwner, WorkflowAttemptReconnect};
use crate::workflow::scheduler::{
    ActiveAttempt, ActiveAttemptPhase, AttemptKey, PendingAttempt, StopDisposition,
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowPreparedDelivery,
    WorkflowPreparedWorker, WorkflowRecoveryAdoptionError, WorkflowRecoveryAdoptionRequest,
    WorkflowSpawnFailure, WorkflowSpawnRequest, WorkflowSpawner, WorkflowStopStatus,
    WorkflowWorkerOutcome,
};
use crate::workflow::state::WorkflowActorState;
use crate::workflow::terminal_delivery::UnavailableWorkflowTerminalSink;
use crate::workflow::transition::apply_payload;
use crate::workflow::{
    SystemWorkflowIdSource, WorkflowClock, WorkflowCoordinatorError, WorkflowOwner,
};

include!("journal.rs");
include!("spawner.rs");
include!("runs.rs");
include!("coordinator.rs");
include!("worker.rs");

mod abort;
mod activation;
mod drain;
mod pending_stop;
mod preparation;
mod preparation_edges;
mod recovery_adoption;
mod reservation_delivery;
mod reservation_guards;
mod resume;
mod stop_effect_edges;
mod stop_pending_edges;
mod stop_wrappers;
