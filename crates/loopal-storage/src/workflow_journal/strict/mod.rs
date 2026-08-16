mod common;
mod event;
pub(crate) mod request;
mod snapshot;

use loopal_protocol::{WorkflowTerminalDeliveryId, WorkflowTerminalNotification};
use serde::Deserialize;

use super::WorkflowJournalError;
use super::record::{WORKFLOW_JOURNAL_VERSION, WorkflowJournalEntry};
pub(crate) use event::StrictEvent;
pub(crate) use snapshot::StrictSnapshot;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum StrictEntry {
    Init {
        version: u16,
        snapshot: Box<StrictSnapshot>,
        #[serde(default)]
        events: Vec<StrictEvent>,
        request: Option<StrictRequestRecord>,
    },
    Commit {
        version: u16,
        run_id: String,
        events: Vec<StrictEvent>,
        request: Option<StrictRequestRecord>,
    },
    DeliveryIntent {
        version: u16,
        notification: WorkflowTerminalNotification,
    },
    DeliveryAck {
        version: u16,
        delivery_id: StrictDeliveryId,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictDeliveryId {
    session_id: String,
    run_id: String,
    terminal_revision: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictRequestRecord {
    request_id: String,
    operation: String,
    payload: serde_json::Value,
    response: serde_json::Value,
}

impl TryFrom<StrictEntry> for WorkflowJournalEntry {
    type Error = WorkflowJournalError;

    fn try_from(value: StrictEntry) -> Result<Self, Self::Error> {
        match value {
            StrictEntry::Init {
                version,
                snapshot,
                events,
                request,
            } => Ok(Self::Init {
                version: checked_version(version)?,
                snapshot: Box::new((*snapshot).into()),
                events: events.into_iter().map(Into::into).collect(),
                request: request.map(Into::into),
            }),
            StrictEntry::Commit {
                version,
                run_id,
                events,
                request,
            } => Ok(Self::Commit {
                version: checked_version(version)?,
                run_id: run_id.into(),
                events: events.into_iter().map(Into::into).collect(),
                request: request.map(Into::into),
            }),
            StrictEntry::DeliveryIntent {
                version,
                notification,
            } => Ok(Self::DeliveryIntent {
                version: checked_version(version)?,
                notification,
            }),
            StrictEntry::DeliveryAck {
                version,
                delivery_id,
            } => Ok(Self::DeliveryAck {
                version: checked_version(version)?,
                delivery_id: delivery_id.into(),
            }),
        }
    }
}

impl From<StrictDeliveryId> for WorkflowTerminalDeliveryId {
    fn from(value: StrictDeliveryId) -> Self {
        Self::new(
            value.session_id,
            value.run_id.into(),
            value.terminal_revision,
        )
    }
}

fn checked_version(version: u16) -> Result<u16, WorkflowJournalError> {
    if version == WORKFLOW_JOURNAL_VERSION {
        Ok(version)
    } else {
        Err(WorkflowJournalError::Corruption {
            path: std::path::PathBuf::new(),
            offset: 0,
            detail: format!("unsupported version {version}"),
        })
    }
}
