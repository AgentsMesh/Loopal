use loopal_protocol::{
    WorkflowEvent, WorkflowRequestRecord, WorkflowRunId, WorkflowRunSnapshot,
    WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
};
use serde::{Deserialize, Serialize};

pub const WORKFLOW_JOURNAL_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowJournalEntry {
    Init {
        version: u16,
        snapshot: Box<WorkflowRunSnapshot>,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    },
    Commit {
        version: u16,
        run_id: WorkflowRunId,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    },
    DeliveryIntent {
        version: u16,
        notification: WorkflowTerminalNotification,
    },
    DeliveryAck {
        version: u16,
        delivery_id: WorkflowTerminalDeliveryId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowJournalInit {
    pub snapshot: WorkflowRunSnapshot,
    pub events: Vec<WorkflowEvent>,
    pub request: Option<WorkflowRequestRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowJournalCommit {
    pub run_id: WorkflowRunId,
    pub events: Vec<WorkflowEvent>,
    pub request: Option<WorkflowRequestRecord>,
}

impl WorkflowJournalEntry {
    pub(crate) fn init(
        snapshot: WorkflowRunSnapshot,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    ) -> Self {
        Self::Init {
            version: WORKFLOW_JOURNAL_VERSION,
            snapshot: Box::new(snapshot),
            events,
            request,
        }
    }

    pub(crate) fn commit(
        run_id: WorkflowRunId,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    ) -> Self {
        Self::Commit {
            version: WORKFLOW_JOURNAL_VERSION,
            run_id,
            events,
            request,
        }
    }

    pub(crate) fn delivery_ack(delivery_id: WorkflowTerminalDeliveryId) -> Self {
        Self::DeliveryAck {
            version: WORKFLOW_JOURNAL_VERSION,
            delivery_id,
        }
    }

    pub(crate) fn delivery_intent(notification: WorkflowTerminalNotification) -> Self {
        Self::DeliveryIntent {
            version: WORKFLOW_JOURNAL_VERSION,
            notification,
        }
    }

    pub(crate) fn version(&self) -> u16 {
        match self {
            Self::Init { version, .. }
            | Self::Commit { version, .. }
            | Self::DeliveryIntent { version, .. }
            | Self::DeliveryAck { version, .. } => *version,
        }
    }
}
