use std::collections::HashMap;

use loopal_protocol::{
    WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::{WorkflowCleanupStatus, WorkflowWorkerOutcome};
use crate::types::AgentExecutionRef;
use crate::workflow::WorkflowOwner;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::workflow) struct AttemptKey {
    pub(in crate::workflow) run_id: WorkflowRunId,
    pub(in crate::workflow) node_id: WorkflowNodeId,
    pub(in crate::workflow) attempt_id: WorkflowAttemptId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::workflow) enum ActiveAttemptPhase {
    Activating,
    Running,
    Interrupting,
    ShuttingDown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::workflow) enum StopDisposition {
    Cancelled(String),
    Failed(super::WorkflowSpawnFailure),
}

pub(in crate::workflow) struct ActiveAttempt {
    pub(in crate::workflow) owner: WorkflowOwner,
    pub(in crate::workflow) key: AttemptKey,
    pub(in crate::workflow) execution: AgentExecutionRef,
    pub(in crate::workflow) outcome: Option<oneshot::Receiver<WorkflowWorkerOutcome>>,
    pub(in crate::workflow) outcome_waiter: Option<JoinHandle<()>>,
    pub(in crate::workflow) shutdown_waiter: Option<JoinHandle<WorkflowCleanupStatus>>,
    pub(in crate::workflow) deadline_unix_ms: u64,
    pub(in crate::workflow) shutdown_after_unix_ms: Option<u64>,
    pub(in crate::workflow) phase: ActiveAttemptPhase,
    pub(in crate::workflow) stop: Option<StopDisposition>,
}

impl Drop for ActiveAttempt {
    fn drop(&mut self) {
        if let Some(waiter) = self.outcome_waiter.take() {
            waiter.abort();
        }
        // A shutdown waiter is an exact-lease containment supervisor. Drop
        // detaches it instead of aborting it: removing the scheduler record
        // must not cancel cleanup before the adapter has acknowledged the
        // generation-bound shutdown (especially during owner quarantine).
        let _ = self.shutdown_waiter.take();
    }
}

pub(in crate::workflow) struct PendingAttempt {
    pub(in crate::workflow) owner: WorkflowOwner,
    pub(in crate::workflow) key: AttemptKey,
    pub(in crate::workflow) causation: WorkflowPermissionCausation,
    pub(in crate::workflow) deadline_unix_ms: u64,
    pub(in crate::workflow) prepare_abort: Option<JoinHandle<()>>,
    pub(in crate::workflow) abort_waiter: Option<JoinHandle<WorkflowCleanupStatus>>,
    pub(in crate::workflow) abort_requested: bool,
    pub(in crate::workflow) abort_status: Option<WorkflowCleanupStatus>,
    pub(in crate::workflow) delivery_finished: bool,
    pub(in crate::workflow) late_execution: Option<AgentExecutionRef>,
    pub(in crate::workflow) late_shutdown_waiter: Option<JoinHandle<WorkflowCleanupStatus>>,
    pub(in crate::workflow) stop: Option<StopDisposition>,
}

impl Drop for PendingAttempt {
    fn drop(&mut self) {
        if let Some(task) = self.prepare_abort.take() {
            task.abort();
        }
        // The abort waiter owns the external preparation tombstone. Keep the
        // bounded supervisor alive after the in-memory pending record is
        // removed so a late registration cannot escape containment.
        let _ = self.abort_waiter.take();
        // A late exact lease has the same ownership rule: dropping its
        // tombstone detaches the one shutdown supervisor instead of aborting it.
        let _ = self.late_shutdown_waiter.take();
    }
}

pub(in crate::workflow) type ActiveAttempts = HashMap<WorkflowAttemptId, ActiveAttempt>;
pub(in crate::workflow) type PendingAttempts = HashMap<WorkflowAttemptId, PendingAttempt>;
